use alloc::vec::Vec;
use alloc::string::String;
use alloc::rc::Rc;
use crate::get_free_page_num;

use crate::interrupt::timer::TimeSpec;
use crate::task::task_scheduler::{add_task_to_scheduler, get_task, get_task_num};
use crate::task::process::Process;
use crate::task::pid::get_next_pid;
use crate::task::task::{Task, Rusage};
use crate::task::exec_with_process;
use crate::runtime_err::RuntimeError;
use crate::memory::addr::UserAddr;

use crate::task::task::TaskStatus;

use super::{UTSname, CloneFlags};
use super::SYS_CALL_ERR;

bitflags! {
    struct FutexFlags: u32 {
        const WAIT      = 0;
        const WAKE      = 1;
        const REQUEUE   = 3;
        const FUTEX_WAKE_OP = 5;
        const LOCK_PI   = 6;
        const UNLOCK_PI = 7;
        const PRIVATE   = 0x80;
    }
}

impl Task {
    /// 退出当前任务 
    pub fn sys_exit(&self, exit_code: usize) -> Result<(), RuntimeError> {
        let inner = self.inner.borrow();
        if self.tid == 0 {
            inner.process.borrow_mut().exit(exit_code);
        } else {
            self.exit();
        }

        let clear_child_tid = self.clear_child_tid.borrow().clone();
        if clear_child_tid.is_valid() {
            *clear_child_tid.transfer() = 0;
        }
        Err(RuntimeError::KillCurrentTask)
    }
    
    // 退出当前进程？ eg: 功能也许有待完善
    pub fn sys_exit_group(&self, exit_code: usize) -> Result<(), RuntimeError> {
        let inner = self.inner.borrow_mut();
        let mut process = inner.process.borrow_mut();
        debug!("exit pid: {}", self.pid);
        process.exit(exit_code);
        match &process.parent {
            Some(parent) => {
                let parent = parent.upgrade().unwrap();
                let _parent = parent.borrow();

                // let end: UserAddr<TimeSpec> = 0x10bb78.into();
                // let start: UserAddr<TimeSpec> = 0x10bad0.into();

                // println!("start: {:?}   end: {:?}",start.transfer(), end.transfer());

                // let target_end: UserAddr<TimeSpec> = parent.pmm.get_phys_addr(0x10bb78usize.into())?.0.into();
                // let target_start: UserAddr<TimeSpec> = parent.pmm.get_phys_addr(0x10bad0usize.into())?.0.into();
                // *target_start.transfer() = *start.transfer();
                // *target_end.transfer() = *end.transfer();

                // let task = parent.tasks[0].clone().upgrade().unwrap();
                // drop(parent);
                // // 处理signal 17 SIGCHLD
                // task.signal(17);
            }
            None => {}
        }
        debug!("剩余页表: {}", get_free_page_num());
        debug!("exit_code: {:#x}", exit_code);
        Err(RuntimeError::ChangeTask)
    }
    
    // 设置 tid addr
    pub fn sys_set_tid_address(&self, tid_ptr: UserAddr<u32>) -> Result<(), RuntimeError> {
        // 测试写入用户空间
        let tid_ptr = tid_ptr.transfer();
        let mut inner = self.inner.borrow_mut();
        let clear_child_tid = self.clear_child_tid.borrow().clone();

        *tid_ptr = if clear_child_tid.is_valid() {
            clear_child_tid.transfer().clone()
        } else {
            0
        };

        inner.context.x[10] = self.tid;
        Ok(())
    }
    
    pub fn sys_sched_yield(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        inner.status = TaskStatus::READY;
        Err(RuntimeError::ChangeTask)
    }
    
    // 获取系统信息
    pub fn sys_uname(&self, ptr: UserAddr<UTSname>) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
    
        // 获取参数
        let sys_info = ptr.transfer();
        // 写入系统信息

        // let sys_name = b"ByteOS";
        // let sys_nodename = b"ByteOS";
        // let sys_release = b"release";
        // let sys_version = b"alpha 1.1";
        // let sys_machine = b"riscv k210";
        // let sys_domain = b"alexbd.cn";
        let sys_name = b"Linux";
        let sys_nodename = b"debian";
        let sys_release = b"5.10.0-7-riscv64";
        let sys_version = b"#1 SMP Debian 5.10.40-1 (2021-05-28)";
        let sys_machine = b"riscv k210";
        let sys_domain = b"alexbd.cn";

        sys_info.sysname[..sys_name.len()].copy_from_slice(sys_name);
        sys_info.nodename[..sys_nodename.len()].copy_from_slice(sys_nodename);
        sys_info.release[..sys_release.len()].copy_from_slice(sys_release);
        sys_info.version[..sys_version.len()].copy_from_slice(sys_version);
        sys_info.machine[..sys_machine.len()].copy_from_slice(sys_machine);
        sys_info.domainname[..sys_domain.len()].copy_from_slice(sys_domain);
        inner.context.x[10] = 0;
        Ok(())
    }
    
    // 获取pid
    pub fn sys_getpid(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        inner.context.x[10] = self.pid;
        Ok(())
    }
    
    // 获取父id
    pub fn sys_getppid(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        let process = inner.process.clone();
        let process = process.borrow();

        inner.context.x[10] = match &process.parent {
            Some(parent) => {
                let parent = parent.upgrade().unwrap();
                let x = parent.borrow().pid; 
                x
            },
            None => SYS_CALL_ERR
        };

        Ok(())
    }
    
    // 获取线程id
    pub fn sys_gettid(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        inner.context.x[10] = self.tid;
        Ok(())
    }
    
    // fork process
    pub fn sys_fork(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        let process = inner.process.clone();
        let mut process = process.borrow_mut();

        let (child_process, child_task) =
            Process::new(get_next_pid(), Some(Rc::downgrade(&inner.process)))?;
        process.children.push(child_process.clone());
        let mut child_task_inner = child_task.inner.borrow_mut();
        child_task_inner.context.clone_from(&inner.context);
        child_task_inner.context.x[10] = 0;
        drop(child_task_inner);
        add_task_to_scheduler(child_task.clone());
        let cpid = child_task.pid;
        inner.context.x[10] = cpid;
        let mut child_process = child_process.borrow_mut();
        child_process.mem_set = process.mem_set.clone_with_data()?;
        child_process.stack = process.stack.clone_with_data(child_process.pmm.clone())?;
        // 复制fd_table
        child_process.fd_table = process.fd_table.clone();
        // 创建新的heap
        // child_process.heap = UserHeap::new(child_process.pmm.clone())?;
        child_process.heap = process.heap.clone_with_data(child_process.pmm.clone())?;
        debug!("heap_pointer: {:#x}", child_process.heap.get_heap_top());
        child_process.pmm.add_mapping_by_set(&child_process.mem_set)?;
        drop(process);
        drop(child_process);
        drop(inner);
        // Ok(())
        Err(RuntimeError::ChangeTask)
    }

    pub fn sys_spec_fork(&self, flags: usize, _new_sp: usize, _ptid: UserAddr<u32>, _tls: usize, ctid_ptr: UserAddr<u32>) -> Result<(), RuntimeError>{
        // return self.sys_fork();
        let flags = CloneFlags::from_bits_truncate(flags);
        let mut inner = self.inner.borrow_mut();
        let process = inner.process.clone();

        let cpid = get_next_pid();
        let (child_process, child_task) =
            Process::fork(cpid, process.clone())?;
        
        let mut process = process.borrow_mut();
        process.children.push(child_process.clone());

        let mut child_task_inner = child_task.inner.borrow_mut();
        child_task_inner.context.clone_from(&inner.context);
        child_task_inner.context.x[10] = 0;
        drop(child_task_inner);

        add_task_to_scheduler(child_task.clone());

        let mut child_process = child_process.borrow_mut();
        child_process.stack = process.stack.clone_with_data(child_process.pmm.clone())?;
        // child_process.heap = process.heap.clone_with_data(child_process.pmm.clone())?;
        inner.context.x[10] = cpid;

        if flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
            *ctid_ptr.transfer() = cpid as u32;
        }

        if flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
            
        }

        drop(process);
        drop(child_process);
        drop(inner);
        // Ok(())
        Err(RuntimeError::ChangeTask)
    }
    
    // clone task
    pub fn sys_clone(&self, flags: usize, new_sp: usize, ptid: UserAddr<u32>, tls: usize, ctid_ptr: UserAddr<u32>) -> Result<(), RuntimeError> {
        // let flags = flags & 0x4fff;
        debug!(
            "clone: flags={:#x}, newsp={:#x}, parent_tid={:#x}, child_tid={:#x}, newtls={:#x}",
            flags, new_sp, ptid.bits(), ctid_ptr.0 as usize, tls
        );
        if flags == 0x4111 || flags == 0x11 {
            // VFORK | VM | SIGCHILD
            warn!("sys_clone is calling sys_fork instead, ignoring other args");
            return self.sys_fork();
        } else if flags == 0x1200011 {
            return self.sys_spec_fork(0x11, new_sp, ptid, tls, ctid_ptr);
            // return self.sys_fork();
        }

        debug!(
            "clone: flags={:#x}, newsp={:#x}, parent_tid={:#x}, child_tid={:#x}, newtls={:#x}",
            flags, new_sp, ptid.bits(), ctid_ptr.0 as usize, tls
        );

        let mut inner = self.inner.borrow_mut();
        let process = inner.process.clone();
        let process = process.borrow();
        
        let ctid = process.tasks.len();
        drop(process);

        let new_task = Task::new(ctid, inner.process.clone());
        let mut new_task_inner = new_task.inner.borrow_mut();
        new_task_inner.context.clone_from(&inner.context);
        new_task_inner.context.x[2] = new_sp;
        new_task_inner.context.x[4] = tls;
        new_task_inner.context.x[10] = 0;
        add_task_to_scheduler(new_task.clone());
        // 添加到process
        inner.context.x[10] = ctid;
        
        debug!("tasks: len {}", get_task_num());

        drop(new_task_inner);
        drop(inner);
        if ptid.is_valid() {
            *ptid.transfer() = ctid as u32;
        }
        // 执行 set_tid_address
        new_task.set_tid_address(ctid_ptr);
        // just finish clone, not change task
        Ok(())
        // Err(RuntimeError::ChangeTask)
    }

    // 执行文件
    pub fn sys_execve(&self, filename: UserAddr<u8>, argv: UserAddr<UserAddr<u8>>, 
            _envp: UserAddr<UserAddr<u8>>) -> Result<(), RuntimeError> {
        let inner = self.inner.borrow_mut();
        let mut process = inner.process.borrow_mut();
        let filename = filename.read_string();

        debug!("run {}", filename);
        let args = argv.transfer_until(|x| !x.is_valid());
        let args:Vec<String> = args.iter_mut().map(|x| x.read_string()).collect();

        // 读取envp
        // let envp = argv.translate_until(pmm.clone(), |x| !x.is_valid());
        // let envp:Vec<String> = envp.iter_mut().map(|x| x.read_string(pmm.clone())).collect();

        // 获取 envp
        let task = process.tasks[self.tid].clone().upgrade().unwrap();
        process.reset()?;
        drop(process);
        let process = inner.process.clone();
        drop(inner);
        exec_with_process(process.clone(), task, &filename, args.iter().map(AsRef::as_ref).collect())?;
        // process.borrow_mut().new_heap()?;
        self.before_run();
        Ok(())
    }
    
    // wait task
    pub fn sys_wait4(&self, pid: usize, ptr: UserAddr<i32>, _options: usize) -> Result<(), RuntimeError> {
        debug!("pid: {:#x}, ptr: {:#x}, _options: {}", pid, ptr.bits(), _options);
        let mut inner = self.inner.borrow_mut();
        let process = inner.process.clone();
        let mut process = process.borrow_mut();


        if pid != SYS_CALL_ERR {
            let target = 
            process.children.iter().find(|&x| x.borrow().pid == pid);

            if let Some(exit_code) = target.map_or(None, |x| x.borrow().exit_code) {
                if ptr.is_valid() {
                    *ptr.transfer() = exit_code as i32;
                }

                inner.context.x[10] = pid;
                return Ok(())
            }
        } else {
            if process.children.len() == 0 {
                inner.context.x[10] = -10 as isize as usize;
                return Ok(());
            }

            let cprocess_vec = 
                process.children.drain_filter(|x| x.borrow().exit_code.is_some()).collect::<Vec<_>>();

            debug!("cpro len: {}", cprocess_vec.len());

            if cprocess_vec.len() != 0 {
                let cprocess = cprocess_vec[0].borrow();
                if ptr.is_valid() {
                    *ptr.transfer() = cprocess.exit_code.unwrap() as i32;
                }
                inner.context.x[10] = cprocess.pid;
                return Ok(());
            }
        }
        inner.context.sepc -= 4;
        drop(process);
        drop(inner);
        Err(RuntimeError::ChangeTask)
    }
    
    // kill task
    pub fn sys_kill(&self, _pid: usize, _signum: usize) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        debug!(
            "kill: thread {} kill process {} with signal {:?}",
            0,
            _pid,
            _signum
        );

        inner.context.x[10] = 0;
        Ok(())
    }

    // wait for futex
    pub fn sys_futex(&self, uaddr: UserAddr<i32>, op: u32, value: i32, _value2: usize, _value3: usize) -> Result<(), RuntimeError> {
        debug!("sys_futex uaddr: {:#x} op: {:#x} value: {:#x}", uaddr.bits(), op, value);
        // let uaddr_ref = uaddr.transfer();
        // let op = FutexFlags::from_bits_truncate(op);
        // let mut inner = self.inner.borrow_mut();
        // let process = inner.process.borrow_mut();

        // let op = op - FutexFlags::PRIVATE;
        // debug!(
        //     "Futex uaddr: {:#x}, op: {:?}, val: {:#x}, val2(timeout_addr): {:x}",
        //     uaddr.bits(), op, value, value2,
        // );
        // match op {
        //     FutexFlags::WAIT => {
        //         if *uaddr_ref == value {
        //             drop(process);
        //             debug!("等待进程");
        //             inner.context.x[10] = 0;
        //             inner.status = TaskStatus::WAITING;
        //             drop(inner);
        //             // futex_wait(uaddr.bits());
        //             switch_next();
        //         } else {
        //             // *uaddr_value -= 1;
        //             drop(process);
        //             inner.context.x[10] = 0;
        //         }
        //     },
        //     FutexFlags::WAKE => {
        //         // *uaddr_value = -1;
        //         drop(process);
        //         debug!("debug for ");
        //         // 值为唤醒的线程数
        //         // let count = futex_wake(uaddr.bits(), value as usize);
        //         // inner.context.x[10] = count;
        //         // debug!("wake count : {}", count);
        //         drop(inner);
        //         switch_next();
        //     }
        //     FutexFlags::REQUEUE => {
        //         drop(process);
        //         inner.context.x[10] = 0;

        //     }
        //     _ => todo!(),
        // }
        // if op.contains(FutexFlags::WAKE) {
        //     // *uaddr_value = 0;
        //     // futex_requeue(uaddr.bits(), value as u32, value2, value3 as u32);
        // }
        Ok(())
    }

    // kill task
    pub fn sys_tkill(&self, tid: usize, signum: usize) -> Result<(), RuntimeError> {
        let mut inner = self.inner.borrow_mut();
        inner.context.x[10] = 0;
        let signal_task = get_task(self.pid, tid);
        debug!("signum: {}", signum);
        if let Some(signal_task) = signal_task {
            drop(inner);
            signal_task.signal(signum)?;
        }
        Ok(())
    }

    pub fn sys_tgkill(&self, tgid: usize, tid: usize, signum: usize) -> Result<(), RuntimeError> {
        debug!("tgkill: tgid: {}  tid: {}  signum {}", tgid, tid, signum);
        if let Some(task) = get_task(tgid, tid) {
            task.signal(signum)?;
        } else {
            self.update_context(|x| x.x[10] = SYS_CALL_ERR);
        }
        Ok(())
    }

    pub fn sys_getrusage(&self, _who: usize, usage: UserAddr<Rusage>) -> Result<(), RuntimeError>{
        let mut inner = self.inner.borrow_mut();
        let usage = usage.transfer();
        usage.ru_stime = TimeSpec::now();
        usage.ru_utime = TimeSpec::now();
        inner.context.x[10] = SYS_CALL_ERR;
        Ok(())
    }
}
