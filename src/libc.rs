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
  EPERM,
  ENOENT,
  ESRCH,
  EINTR,
  EIO,
  ENXIO,
  E2BIG,
  ENOEXEC,
  EBADF,
  ECHILD,
  EAGAIN,
  ENOMEM,
  EACCES,
  EFAULT,
  ENOTBLK,
  EBUSY,
  EEXIST,
  EXDEV,
  ENODEV,
  ENOTDIR,
  EISDIR,
  EINVAL,
  ENFILE,
  EMFILE,
  ENOTTY,
  ETXTBSY,
  EFBIG,
  ENOSPC,
  ESPIPE,
  EROFS,
  EMLINK,
  EPIPE,
  EDOM,
  ERANGE,
  EDEADLK,
  ENAMETOOLONG,
  ENOLCK,
  ENOSYS,
  ENOTEMPTY,
  ELOOP,
  ENOMSG,
  EIDRM,
  ECHRNG,
  EL2NSYNC,
  EL3HLT,
  EL3RST,
  ELNRNG,
  EUNATCH,
  ENOCSI,
  EL2HLT,
  EBADE,
  EBADR,
  EXFULL,
  ENOANO,
  EBADRQC,
  EBADSLT,
  EBFONT,
  ENOSTR,
  ENODATA,
  ETIME,
  ENOSR,
  ENONET,
  ENOPKG,
  EREMOTE,
  ENOLINK,
  EADV,
  ESRMNT,
  ECOMM,
  EPROTO,
  EMULTIHOP,
  EDOTDOT,
  EBADMSG,
  EOVERFLOW,
  ENOTUNIQ,
  EBADFD,
  EREMCHG,
  ELIBACC,
  ELIBBAD,
  ELIBSCN,
  ELIBMAX,
  ELIBEXEC,
  EILSEQ,
  ERESTART,
  ESTRPIPE,
  EUSERS,
  ENOTSOCK,
  EDESTADDRREQ,
  EMSGSIZE,
  EPROTOTYPE,
  ENOPROTOOPT,
  EPROTONOSUPPORT,
  ESOCKTNOSUPPORT,
  EOPNOTSUPP,
  EPFNOSUPPORT,
  EAFNOSUPPORT,
  EADDRINUSE,
  EADDRNOTAVAIL,
  ENETDOWN,
  ENETUNREACH,
  ENETRESET,
  ECONNABORTED,
  ECONNRESET,
  ENOBUFS,
  EISCONN,
  ENOTCONN,
  ESHUTDOWN,
  ETOOMANYREFS,
  ETIMEDOUT,
  ECONNREFUSED,
  EHOSTDOWN,
  EHOSTUNREACH,
  EALREADY,
  EINPROGRESS,
  ESTALE,
  EUCLEAN,
  ENOTNAM,
  ENAVAIL,
  EISNAM,
  EREMOTEIO,
  EDQUOT,
  ENOMEDIUM,
  EMEDIUMTYPE,
  ECANCELED,
  ENOKEY,
  EKEYEXPIRED,
  EKEYREVOKED,
  EKEYREJECTED,
  EOWNERDEAD,
  ENOTRECOVERABLE,
  ERFKILL,
  EHWPOISON,
  RIO_UNKNOWN,
}

#[allow(clippy::too_many_lines)]
pub fn errno(e: i32) -> Result<(), Errno> {
  match unsafe { errno_to_int(e) } {
    0 => Result::Ok(()),
    -1 => Result::Err(Errno::EPERM),
    -2 => Result::Err(Errno::ENOENT),
    -3 => Result::Err(Errno::ESRCH),
    -4 => Result::Err(Errno::EINTR),
    -5 => Result::Err(Errno::EIO),
    -6 => Result::Err(Errno::ENXIO),
    -7 => Result::Err(Errno::E2BIG),
    -8 => Result::Err(Errno::ENOEXEC),
    -9 => Result::Err(Errno::EBADF),
    -10 => Result::Err(Errno::ECHILD),
    -11 => Result::Err(Errno::EAGAIN),
    -12 => Result::Err(Errno::ENOMEM),
    -13 => Result::Err(Errno::EACCES),
    -14 => Result::Err(Errno::EFAULT),
    -15 => Result::Err(Errno::ENOTBLK),
    -16 => Result::Err(Errno::EBUSY),
    -17 => Result::Err(Errno::EEXIST),
    -18 => Result::Err(Errno::EXDEV),
    -19 => Result::Err(Errno::ENODEV),
    -20 => Result::Err(Errno::ENOTDIR),
    -21 => Result::Err(Errno::EISDIR),
    -22 => Result::Err(Errno::EINVAL),
    -23 => Result::Err(Errno::ENFILE),
    -24 => Result::Err(Errno::EMFILE),
    -25 => Result::Err(Errno::ENOTTY),
    -26 => Result::Err(Errno::ETXTBSY),
    -27 => Result::Err(Errno::EFBIG),
    -28 => Result::Err(Errno::ENOSPC),
    -29 => Result::Err(Errno::ESPIPE),
    -30 => Result::Err(Errno::EROFS),
    -31 => Result::Err(Errno::EMLINK),
    -32 => Result::Err(Errno::EPIPE),
    -33 => Result::Err(Errno::EDOM),
    -34 => Result::Err(Errno::ERANGE),
    -35 => Result::Err(Errno::EDEADLK),
    -36 => Result::Err(Errno::ENAMETOOLONG),
    -37 => Result::Err(Errno::ENOLCK),
    -38 => Result::Err(Errno::ENOSYS),
    -39 => Result::Err(Errno::ENOTEMPTY),
    -40 => Result::Err(Errno::ELOOP),
    -42 => Result::Err(Errno::ENOMSG),
    -43 => Result::Err(Errno::EIDRM),
    -44 => Result::Err(Errno::ECHRNG),
    -45 => Result::Err(Errno::EL2NSYNC),
    -46 => Result::Err(Errno::EL3HLT),
    -47 => Result::Err(Errno::EL3RST),
    -48 => Result::Err(Errno::ELNRNG),
    -49 => Result::Err(Errno::EUNATCH),
    -50 => Result::Err(Errno::ENOCSI),
    -51 => Result::Err(Errno::EL2HLT),
    -52 => Result::Err(Errno::EBADE),
    -53 => Result::Err(Errno::EBADR),
    -54 => Result::Err(Errno::EXFULL),
    -55 => Result::Err(Errno::ENOANO),
    -56 => Result::Err(Errno::EBADRQC),
    -57 => Result::Err(Errno::EBADSLT),
    -59 => Result::Err(Errno::EBFONT),
    -60 => Result::Err(Errno::ENOSTR),
    -61 => Result::Err(Errno::ENODATA),
    -62 => Result::Err(Errno::ETIME),
    -63 => Result::Err(Errno::ENOSR),
    -64 => Result::Err(Errno::ENONET),
    -65 => Result::Err(Errno::ENOPKG),
    -66 => Result::Err(Errno::EREMOTE),
    -67 => Result::Err(Errno::ENOLINK),
    -68 => Result::Err(Errno::EADV),
    -69 => Result::Err(Errno::ESRMNT),
    -70 => Result::Err(Errno::ECOMM),
    -71 => Result::Err(Errno::EPROTO),
    -72 => Result::Err(Errno::EMULTIHOP),
    -73 => Result::Err(Errno::EDOTDOT),
    -74 => Result::Err(Errno::EBADMSG),
    -75 => Result::Err(Errno::EOVERFLOW),
    -76 => Result::Err(Errno::ENOTUNIQ),
    -77 => Result::Err(Errno::EBADFD),
    -78 => Result::Err(Errno::EREMCHG),
    -79 => Result::Err(Errno::ELIBACC),
    -80 => Result::Err(Errno::ELIBBAD),
    -81 => Result::Err(Errno::ELIBSCN),
    -82 => Result::Err(Errno::ELIBMAX),
    -83 => Result::Err(Errno::ELIBEXEC),
    -84 => Result::Err(Errno::EILSEQ),
    -85 => Result::Err(Errno::ERESTART),
    -86 => Result::Err(Errno::ESTRPIPE),
    -87 => Result::Err(Errno::EUSERS),
    -88 => Result::Err(Errno::ENOTSOCK),
    -89 => Result::Err(Errno::EDESTADDRREQ),
    -90 => Result::Err(Errno::EMSGSIZE),
    -91 => Result::Err(Errno::EPROTOTYPE),
    -92 => Result::Err(Errno::ENOPROTOOPT),
    -93 => Result::Err(Errno::EPROTONOSUPPORT),
    -94 => Result::Err(Errno::ESOCKTNOSUPPORT),
    -95 => Result::Err(Errno::EOPNOTSUPP),
    -96 => Result::Err(Errno::EPFNOSUPPORT),
    -97 => Result::Err(Errno::EAFNOSUPPORT),
    -98 => Result::Err(Errno::EADDRINUSE),
    -99 => Result::Err(Errno::EADDRNOTAVAIL),
    -100 => Result::Err(Errno::ENETDOWN),
    -101 => Result::Err(Errno::ENETUNREACH),
    -102 => Result::Err(Errno::ENETRESET),
    -103 => Result::Err(Errno::ECONNABORTED),
    -104 => Result::Err(Errno::ECONNRESET),
    -105 => Result::Err(Errno::ENOBUFS),
    -106 => Result::Err(Errno::EISCONN),
    -107 => Result::Err(Errno::ENOTCONN),
    -108 => Result::Err(Errno::ESHUTDOWN),
    -109 => Result::Err(Errno::ETOOMANYREFS),
    -110 => Result::Err(Errno::ETIMEDOUT),
    -111 => Result::Err(Errno::ECONNREFUSED),
    -112 => Result::Err(Errno::EHOSTDOWN),
    -113 => Result::Err(Errno::EHOSTUNREACH),
    -114 => Result::Err(Errno::EALREADY),
    -115 => Result::Err(Errno::EINPROGRESS),
    -116 => Result::Err(Errno::ESTALE),
    -117 => Result::Err(Errno::EUCLEAN),
    -118 => Result::Err(Errno::ENOTNAM),
    -119 => Result::Err(Errno::ENAVAIL),
    -120 => Result::Err(Errno::EISNAM),
    -121 => Result::Err(Errno::EREMOTEIO),
    -122 => Result::Err(Errno::EDQUOT),
    -123 => Result::Err(Errno::ENOMEDIUM),
    -124 => Result::Err(Errno::EMEDIUMTYPE),
    -125 => Result::Err(Errno::ECANCELED),
    -126 => Result::Err(Errno::ENOKEY),
    -127 => Result::Err(Errno::EKEYEXPIRED),
    -128 => Result::Err(Errno::EKEYREVOKED),
    -129 => Result::Err(Errno::EKEYREJECTED),
    -130 => Result::Err(Errno::EOWNERDEAD),
    -131 => Result::Err(Errno::ENOTRECOVERABLE),
    -132 => Result::Err(Errno::ERFKILL),
    -133 => Result::Err(Errno::EHWPOISON),
    _ => Result::Err(Errno::RIO_UNKNOWN),
  }
}

impl std::fmt::Debug for Errno {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ECANCELED => f.write_str("ecanceled"),
      Self::RIO_UNKNOWN => f.write_str("ecountered an unkown error"),
      _ => f.write_str("Unsupported error code"),
    }
  }
}
