use alloc::rc::Rc;
use hashbrown::HashMap;
use crate::fs::file::FileOP;
use crate::fs::file::File;
use crate::fs::stdio::StdIn;
use crate::fs::stdio::StdOut;
use crate::fs::stdio::StdErr;
use crate::runtime_err::RuntimeError;
use crate::memory::addr::UserAddr;
use crate::sys_call::consts::EMFILE;

pub const FD_NULL: usize = 0xffffffffffffff9c;
pub const FD_CWD: usize = -100 as isize as usize;
pub const FD_RANDOM: usize = usize::MAX;

#[repr(C)]
#[derive(Clone)]
pub struct IoVec {
    pub iov_base: UserAddr<u8>,
    pub iov_len: usize
}

#[derive(Clone)]
pub struct FDTable(HashMap<usize, Rc<dyn FileOP>>);

impl FDTable {
    pub fn new() -> Self {
        let mut map:HashMap<usize, Rc<dyn FileOP>> = HashMap::new();
        map.insert(0, Rc::new(StdIn));
        map.insert(1, Rc::new(StdOut));
        map.insert(2, Rc::new(StdErr));
        Self(map)
    }

    // 申请fd
    pub fn alloc(&mut self) -> usize {
        (0..).find(|fd| !self.0.contains_key(fd)).unwrap()
    }

    // 申请fd
    pub fn alloc_sock(&mut self) -> usize {
        (50..).find(|fd| !self.0.contains_key(fd)).unwrap()
    }

    // 释放fd
    pub fn dealloc(&mut self, index: usize) {
        self.0.remove(&index);
    }

    // 获取fd内容
    pub fn get(&self, index: usize) -> Result<Rc<dyn FileOP>, RuntimeError> {
        self.0.get(&index).cloned().ok_or(RuntimeError::NoMatchedFileDesc)
    }

    // 获取fd内容
    pub fn get_file(&self, index: usize) -> Result<Rc<File>, RuntimeError> {
        let value = self.0.get(&index).cloned().ok_or(RuntimeError::NoMatchedFileDesc)?;
        value.downcast::<File>().map_err(|_| RuntimeError::NoMatchedFile)
    }

    // 设置fd内容
    pub fn set(&mut self, index: usize, value: Rc<dyn FileOP>) {
        self.0.insert(index, value);
    }

    // 加入描述符
    pub fn push(&mut self, value: Rc<dyn FileOP>) -> usize {
        let index = self.alloc();
        // if index > 41 { return EMFILE; }
        self.set(index, value);
        index
    }

    // 加入描述符
    pub fn push_sock(&mut self, value: Rc<dyn FileOP>) -> usize {
        let index = self.alloc_sock();
        self.set(index, value);
        index
    }
}