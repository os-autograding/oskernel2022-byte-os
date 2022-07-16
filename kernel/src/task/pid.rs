use crate::sync::mutex::Mutex;

// PID生成器
pub struct PidGenerater(usize);

impl PidGenerater {
    // 创建进程id生成器
    pub fn new() -> Self {
        PidGenerater(1000)
    }
    // 切换到下一个pid
    pub fn next(&mut self) -> usize {
        let n = self.0;
        self.0 = n + 1;
        n
    }
}