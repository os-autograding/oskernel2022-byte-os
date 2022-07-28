use core::cell::RefCell;

use alloc::{vec::Vec, rc::{Rc, Weak}};

use crate::{memory::{page_table::PageMappingManager, mem_set::MemSet, addr::VirtAddr}, runtime_err::RuntimeError, interrupt::timer::TMS, fs::filetree::INode};

use super::{task::{Task, TaskStatus}, stack::UserStack, UserHeap, fd_table::FDTable, task_scheduler::kill_process, signal::SigAction};

pub struct Process {
    pub pid: usize,                             // 进程id
    pub parent: Option<Weak<RefCell<Process>>>, // 父进程
    pub pmm: Rc<PageMappingManager>,            // 内存页映射管理 
    pub mem_set: MemSet,                        // 内存使用集
    pub tasks: Vec<Weak<Task>>,                 // 任务管理器
    pub entry: VirtAddr,                        // 入口地址
    pub stack: UserStack,                       // 用户栈
    pub heap: UserHeap,                         // 用户堆
    pub workspace: Rc<INode>,                   // 工作目录
    pub fd_table: FDTable,                      // 文件描述表
    pub tms: TMS,                               // 时间记录结构
    pub signal: SigAction,                      // 信号结构
    pub children: Vec<Rc<RefCell<Process>>>,    // 子结构
    pub exit_code: Option<usize>                // 退出代码
}

impl Process {
    pub fn new(pid: usize, parent: Option<Weak<RefCell<Process>>>)
        -> Result<(Rc<RefCell<Process>>, Rc<Task>), RuntimeError> {
        let pmm = Rc::new(PageMappingManager::new()?);
        let heap = UserHeap::new(pmm.clone())?;
        let process = Self { 
            pid, 
            parent, 
            pmm: pmm.clone(), 
            mem_set: MemSet::new(), 
            tasks: vec![], 
            entry: 0usize.into(), 
            stack: UserStack::new(pmm.clone())?, 
            heap, 
            workspace: INode::get(None, "/", false)?.clone(), 
            fd_table: FDTable::new(),
            children: vec![],
            signal: SigAction::new(),
            tms: TMS::new(),
            exit_code: None
        };
        // 创建默认任务
        let process = Rc::new(RefCell::new(process));
        let task = Task::new(0, process.clone());
        process.borrow_mut().tasks.push(Rc::downgrade(&task));
        Ok((process, task))
    }

    // 进程进行等待
    pub fn wait(&self) {
        let task = self.get_task(0);
        // task.inner.borrow_mut().status = TaskStatus::WAITING;
    }

    // 判断是否在等待状态
    pub fn is_waiting(&self) -> bool {
        // tasks的len 一定大于 0
        let task = self.get_task(0);
        // 如果父进程在等待 则直接释放资源 并改变父进程的状态
        if task.inner.borrow().status == TaskStatus::WAITING {
            true
        } else {
            false
        }
    }

    // 获取task 任务
    pub fn get_task(&self, index: usize) -> Rc<Task> {
        if index >= self.tasks.len() {
            panic!("in process.rs index >= task.len()");
        }
        self.tasks[0].upgrade().unwrap()
    }

    // 结束进程
    pub fn exit(&mut self, exit_code: usize) {
        // 如果没有子进程
        self.exit_code = Some(exit_code);
        // 进程回收
        kill_process(self.pid);
    }

    // 重置内存信息
    pub fn reset(&mut self) -> Result<(), RuntimeError>{
        let pmm = Rc::new(PageMappingManager::new()?);
        let mem_set = MemSet::new();
        self.pmm = pmm;
        self.mem_set = mem_set;
        self.stack = UserStack::new(self.pmm.clone())?;
        Ok(())
    }

    // 释放内存
    pub fn release(&mut self) {
        self.stack.release();
        self.mem_set.release();
        self.pmm.release();
    }
}
    