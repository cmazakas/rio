#[repr(u8)]
pub enum c_void {
  _x1,
}

extern "C" {
  pub fn close(fd: i32) -> i32;
  pub fn errno_to_int(e: i32) -> i32;
  pub fn write(fd: i32, buf: *const c_void, count: usize);
}

pub enum Errno {
  ECANCELED,
  UNKNOWN,
}

impl std::fmt::Debug for Errno {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ECANCELED => f.write_str("ecanceled"),
      Self::UNKNOWN => f.write_str("ecountered an unkown error"),
    }
  }
}

#[must_use]
pub fn errno(e: i32) -> Errno {
  let int = unsafe { errno_to_int(e) };
  match int {
    -1 => Errno::ECANCELED,
    _ => Errno::UNKNOWN,
  }
}
