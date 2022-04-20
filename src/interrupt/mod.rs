//! 中断模块
//! 
//! 

mod handler;
mod context;
pub mod timer;

/// 初始化中断相关的子模块
/// 
/// - [`handler::init`]
/// - [`timer::init`]
pub fn init() {
    handler::init();
    timer::init();
    info!("mod interrupt initialized");
}