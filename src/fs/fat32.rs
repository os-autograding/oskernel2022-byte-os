use core::mem::size_of;

use alloc::borrow::ToOwned;
use alloc::string::{String};
use alloc::vec;

use crate::device::BLK_CONTROL;

// 获取文件系统操作接口
pub trait FilesystemItemOperator {
    fn filename(&self) -> String;            // 获取文件名
    fn file_size(&self) -> usize;            // 获取文件大小
    fn start_cluster(&self) -> usize;        // 开始簇
}

#[derive(Default)]
struct FAT32 {
    device_id: usize,
    bpb: FAT32BPB
}

#[derive(Default)]
#[repr(packed)]
pub struct FAT32BPB {
    jmpcode: [u8; 3],       // 跳转代码
    oem: [u8; 8],           // oem 信息
    bytes_per_sector: u16,  // 每扇区字节数
    sectors_per_cluster: u8,// 每簇扇区数
    reserved_sector: u16,   // 保留扇区数 第一个FAT之前的扇区数 包含引导扇区
    fat_number: u8,         // fat表数量
    root_entries: u16,      // 根目录项数 FAT32必须为0
    small_sector: u16,      // 小扇区区数 FAT32必须为0
    media_descriptor: u8,   // 媒体描述符 0xF8标识硬盘 0xF0表示3.5寸软盘
    sectors_per_fat: u16,   // 每FAT扇区数
    sectors_per_track: u16, // 每道扇区数
    number_of_head: u16,    // 磁头数
    hidden_sector: u32,     // 隐藏扇区数
    large_sector: u32,      // 总扇区数
}

pub enum FAT32FileItemAttr {
    RW  = 0,            // 读写
    R   = 1,            // 只读
    HIDDEN = 1 << 1,    // 隐藏
    SYSTEM = 1 << 2,    // 系统文件
    VOLUME = 1 << 3,    // 卷标
    SUBDIR = 1 << 4,    // 子目录
    FILE   = 1 << 5     // 归档
}

// FAT32短文件目录项
#[repr(packed)]
pub struct FAT32shortFileItem {
    filename: [u8; 8],          // 文件名
    ext: [u8; 3],               // 扩展名
    attr: FAT32FileItemAttr,    // 属性
    system_reserved: u8,        // 系统保留
    create_time_10ms: u8,       // 创建时间的10毫秒位
    create_time: u16,           // 创建时间
    create_date: u16,           // 创建日期
    last_access_date: u16,      // 最后访问日期
    start_high: u16,            // 起始簇号的高16位
    last_modify_time: u16,      // 最近修改时间
    last_modify_date: u16,      // 最近修改日期
    start_low: u16,             // 起始簇号的低16位
    len: u32                    // 文件长度
}

impl FilesystemItemOperator for FAT32shortFileItem {
    // 获取文件名
    fn filename(&self) -> String {
        let filename = String::from_utf8_lossy(&self.filename).into_owned();
        // 获取文件名总长度
        let mut filename_size = filename.len();
        // 获取有效文件名长度
        for i in filename.chars().rev() {
            if !i.is_whitespace() { break; }
            filename_size = filename_size - 1;
        }
        // 拼接得到文件名
        filename[..filename_size].to_owned() + "." + &String::from_utf8_lossy(&self.ext).into_owned()
    }

    // 获取文件大小
    fn file_size(&self) -> usize {
        self.len as usize
    }

    // 开始簇
    fn start_cluster(&self) -> usize {
        (self.start_high as usize) << 16 | self.start_low as usize
    }
}

// FAT32长文件目录项
#[repr(packed)]
pub struct FAT32longFileItem {
    attr: FAT32FileItemAttr,        // 属性
    filename: [u8; 10],             // 长目录文件名unicode码
    sign: u8,                       // 长文件名目录项标志, 取值0FH
    system_reserved: u8,            // 系统保留
    verification: u8,               // 校验值
    filename1: [u8; 12],            // 长文件名unicode码
    start: u16,                     // 文件起始簇号
    filename2: [u8; 4]              // 长文件名unicode码
}

impl FAT32BPB {
    // 获取数据扇区号
    fn data_sector(&self) -> usize {
        (self.reserved_sector + self.fat_number as u16 * self.sectors_per_fat) as usize
    }

    // 输出fat32信息
    fn info(&self) {
        info!("变量地址:{:#x}", &(self.jmpcode) as *const _ as usize);
        info!("磁盘大小:{}", self.large_sector * self.bytes_per_sector as u32);
        info!("FAT表数量:{}, 占扇区:{}", self.fat_number, self.fat_number as u16 * self.sectors_per_fat);
        info!("保留扇区数: {}, 地址: {:#x}", self.reserved_sector, self.reserved_sector * 512);
        info!("数据扇区: {:#x}", self.data_sector());
        info!("OEM信息:{}", String::from_utf8_lossy(&self.oem));
        info!("根目录数量: {:?}", self.jmpcode);
        info!("每簇扇区数: {:#x}, {:#x}", self.sectors_per_cluster, (&self.sectors_per_cluster as *const u8 as usize - self as *const FAT32BPB as usize));
        info!("隐藏扇区数: {:#x}", self.hidden_sector);
    }
}

/// 目前仅支持挂载文件系统
impl FAT32 {
    // 创建新的FAT32表项 device_id: 为设备id 目前支持文件系统 
    fn new(device_id: usize) -> FAT32 {
        let fat32 = FAT32 {
            device_id,
            bpb: Default::default()
        };
        unsafe {
            BLK_CONTROL.read_one_sector(fat32.device_id, 0, &mut *(&fat32.bpb as *const FAT32BPB as *mut [u8; size_of::<FAT32BPB>()]));
        }
        fat32
    }
}

pub fn init() {
    let fat32: FAT32 = FAT32::new(0);
    let mut buf = vec![0u8; 64];
    unsafe {
        fat32.bpb.info();

        BLK_CONTROL.read_one_sector(0, fat32.bpb.data_sector(), &mut buf);
        
        let ref file_item = *(buf.as_mut_ptr() as *mut u8 as *mut FAT32shortFileItem);
        info!("文件名: {}", file_item.filename());
        info!("起始簇: {:#x}, 文件大小: {:#x}", file_item.start_cluster(), file_item.file_size());
        info!("文件起始地址: {:#x}", (fat32.bpb.data_sector() + file_item.start_cluster() * fat32.bpb.sectors_per_cluster as usize) << 9);

        let ref file_item = *(buf.as_mut_ptr().add(32) as *mut u8 as *mut FAT32shortFileItem);
        info!("文件名: {}", file_item.filename());
        info!("起始簇: {:#x}, 文件大小: {:#x}", file_item.start_cluster(), file_item.file_size());
        info!("文件起始地址: {:#x}", (fat32.bpb.data_sector() + (file_item.start_cluster() - 2) * fat32.bpb.sectors_per_cluster as usize) << 9);
    }
}

// 00540003