pub const OP_STRS: &[&str] = &[
    "IORING_OP_NOP",
    "IORING_OP_READV",
    "IORING_OP_WRITEV",
    "IORING_OP_FSYNC",
    "IORING_OP_READ_FIXED",
    "IORING_OP_WRITE_FIXED",
    "IORING_OP_POLL_ADD",
    "IORING_OP_POLL_REMOVE",
    "IORING_OP_SYNC_FILE_RANGE",
    "IORING_OP_SENDMSG",
    "IORING_OP_RECVMSG",
    "IORING_OP_TIMEOUT",
    "IORING_OP_TIMEOUT_REMOVE",
    "IORING_OP_ACCEPT",
    "IORING_OP_ASYNC_CANCEL",
    "IORING_OP_LINK_TIMEOUT",
    "IORING_OP_CONNECT",
    "IORING_OP_FALLOCATE",
    "IORING_OP_OPENAT",
    "IORING_OP_CLOSE",
    "IORING_OP_FILES_UPDATE",
    "IORING_OP_STATX",
    "IORING_OP_READ",
    "IORING_OP_WRITE",
    "IORING_OP_FADVISE",
    "IORING_OP_MADVISE",
    "IORING_OP_SEND",
    "IORING_OP_RECV",
    "IORING_OP_OPENAT2",
    "IORING_OP_EPOLL_CTL",
    "IORING_OP_SPLICE",
    "IORING_OP_PROVIDE_BUFFERS",
    "IORING_OP_REMOVE_BUFFERS",
    "IORING_OP_TEE",
    "IORING_OP_SHUTDOWN",
    "IORING_OP_RENAMEAT",
    "IORING_OP_UNLINKAT",
    "IORING_OP_MKDIRAT",
    "IORING_OP_SYMLINKAT",
    "IORING_OP_LINKAT",
    "IORING_OP_MSG_RING",
];

#[repr(C)]
pub struct io_uring_probe {
    _p: [u8; 0],
}

extern "C" {
    pub fn io_uring_get_probe() -> *mut io_uring_probe;
    pub fn io_uring_free_probe(probe: *mut io_uring_probe);
    pub fn rio_io_uring_opcode_supported(p: *const io_uring_probe, op: i32) -> i32;
}

pub fn print_supported() {
    struct DropGuard {
        probe: *mut io_uring_probe,
    }

    impl Drop for DropGuard {
        fn drop(&mut self) {
            if !self.probe.is_null() {
                unsafe { io_uring_free_probe(self.probe) };
            }
        }
    }

    let guard = DropGuard {
        probe: unsafe { io_uring_get_probe() },
    };

    let probe = guard.probe;
    if probe.is_null() {
        panic!("Unable to acquire io_uring probe");
    }

    println!("Report of your kernel's list of supported io_uring operations:");
    for (idx, op) in OP_STRS.iter().enumerate() {
        print!("{op}: ");
        if unsafe { rio_io_uring_opcode_supported(probe, idx as i32) } > 0 {
            println!("yes!");
        } else {
            println!("no!");
        }
    }
}

#[test]
fn run() {
    print_supported();
}
