#include <liburing.h>

#include <sys/socket.h>
#include <sys/un.h>
#include <sys/timerfd.h>
#include <netinet/in.h>
#include <netinet/ip.h>
#include <arpa/inet.h>

#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct io_uring *rio_setup(unsigned int entries, unsigned int flags)
{
  struct io_uring *ring = malloc(sizeof(*ring));
  int errnum = io_uring_queue_init(entries, ring, flags);
  if (0 != errnum)
  {
    return NULL;
  }
  return ring;
}

void rio_teardown(struct io_uring *ring)
{
  assert(ring != NULL);
  io_uring_queue_exit(ring);
  free(ring);
}

struct io_uring_sqe *rio_make_sqe(struct io_uring *ring)
{
  return io_uring_get_sqe(ring);
}

void rio_io_uring_sqe_set_data(struct io_uring_sqe *sqe, void *data)
{
  io_uring_sqe_set_data(sqe, data);
}

void rio_io_uring_prep_accept(struct io_uring_sqe *sqe, int fd, struct sockaddr *addr, socklen_t *addrlen)
{
  io_uring_prep_accept(sqe, fd, addr, addrlen, 0);
}

void rio_io_uring_prep_read(struct io_uring_sqe *sqe,
                            int fd, void *buf, unsigned nbytes, off_t offset)
{
  return io_uring_prep_read(sqe, fd, buf, nbytes, offset);
}

void rio_io_uring_prep_cancel(struct io_uring_sqe *sqe,
                              void *user_data,
                              int flags)
{
  io_uring_prep_cancel(sqe, user_data, flags);
}

void rio_io_uring_prep_nop(struct io_uring_sqe *sqe)
{
  io_uring_prep_nop(sqe);
}

int rio_io_uring_submit(struct io_uring *ring)
{
  return io_uring_submit(ring);
}

struct io_uring_cqe *rio_io_uring_wait_cqe(struct io_uring *ring, int *res)
{
  struct io_uring_cqe *cqe = NULL;
  int errnum = io_uring_wait_cqe(ring, &cqe);
  if (0 != errnum)
  {
    return NULL;
  }
  *res = cqe->res;
  return cqe;
}

void *rio_io_uring_cqe_get_data(const struct io_uring_cqe *cqe)
{
  return io_uring_cqe_get_data(cqe);
}

void rio_io_uring_cqe_seen(struct io_uring *ring, struct io_uring_cqe *cqe)
{
  io_uring_cqe_seen(ring, cqe);
}

int rio_timerfd_create()
{
  int fd = timerfd_create(CLOCK_REALTIME, 0);
  return fd;
}

int rio_timerfd_settime(int fd, unsigned long secs, unsigned long nanos)
{
  struct timespec now;
  if (-1 == clock_gettime(CLOCK_REALTIME, &now))
  {
    return errno;
  }

  long ns = now.tv_nsec + (1000 * 1000 * 1000) * secs + nanos;

  struct itimerspec expiry;
  expiry.it_value.tv_sec = now.tv_sec + (ns / (1000 * 1000 * 1000));
  expiry.it_value.tv_nsec = ns % (1000 * 1000 * 1000);
  expiry.it_interval.tv_sec = 0;
  expiry.it_interval.tv_nsec = 0;

  if (-1 == timerfd_settime(fd, TFD_TIMER_ABSTIME, &expiry, NULL))
  {
    return errno;
  }

  return 0;
}

int rio_make_pipe(int pipefd[2])
{
  return pipe(pipefd);
}

int errno_to_int(int const e)
{
  switch (e)
  {
  case 0:
    return 0;
  case EPERM:
    return -1;
  case ENOENT:
    return -2;
  case ESRCH:
    return -3;
  case EINTR:
    return -4;
  case EIO:
    return -5;
  case ENXIO:
    return -6;
  case E2BIG:
    return -7;
  case ENOEXEC:
    return -8;
  case EBADF:
    return -9;
  case ECHILD:
    return -10;
  case EAGAIN:
    return -11;
  case ENOMEM:
    return -12;
  case EACCES:
    return -13;
  case EFAULT:
    return -14;
  case ENOTBLK:
    return -15;
  case EBUSY:
    return -16;
  case EEXIST:
    return -17;
  case EXDEV:
    return -18;
  case ENODEV:
    return -19;
  case ENOTDIR:
    return -20;
  case EISDIR:
    return -21;
  case EINVAL:
    return -22;
  case ENFILE:
    return -23;
  case EMFILE:
    return -24;
  case ENOTTY:
    return -25;
  case ETXTBSY:
    return -26;
  case EFBIG:
    return -27;
  case ENOSPC:
    return -28;
  case ESPIPE:
    return -29;
  case EROFS:
    return -30;
  case EMLINK:
    return -31;
  case EPIPE:
    return -32;
  case EDOM:
    return -33;
  case ERANGE:
    return -34;
  case EDEADLK:
    return -35;
  case ENAMETOOLONG:
    return -36;
  case ENOLCK:
    return -37;
  case ENOSYS:
    return -38;
  case ENOTEMPTY:
    return -39;
  case ELOOP:
    return -40;
  case ENOMSG:
    return -42;
  case EIDRM:
    return -43;
  case ECHRNG:
    return -44;
  case EL2NSYNC:
    return -45;
  case EL3HLT:
    return -46;
  case EL3RST:
    return -47;
  case ELNRNG:
    return -48;
  case EUNATCH:
    return -49;
  case ENOCSI:
    return -50;
  case EL2HLT:
    return -51;
  case EBADE:
    return -52;
  case EBADR:
    return -53;
  case EXFULL:
    return -54;
  case ENOANO:
    return -55;
  case EBADRQC:
    return -56;
  case EBADSLT:
    return -57;
  case EBFONT:
    return -59;
  case ENOSTR:
    return -60;
  case ENODATA:
    return -61;
  case ETIME:
    return -62;
  case ENOSR:
    return -63;
  case ENONET:
    return -64;
  case ENOPKG:
    return -65;
  case EREMOTE:
    return -66;
  case ENOLINK:
    return -67;
  case EADV:
    return -68;
  case ESRMNT:
    return -69;
  case ECOMM:
    return -70;
  case EPROTO:
    return -71;
  case EMULTIHOP:
    return -72;
  case EDOTDOT:
    return -73;
  case EBADMSG:
    return -74;
  case EOVERFLOW:
    return -75;
  case ENOTUNIQ:
    return -76;
  case EBADFD:
    return -77;
  case EREMCHG:
    return -78;
  case ELIBACC:
    return -79;
  case ELIBBAD:
    return -80;
  case ELIBSCN:
    return -81;
  case ELIBMAX:
    return -82;
  case ELIBEXEC:
    return -83;
  case EILSEQ:
    return -84;
  case ERESTART:
    return -85;
  case ESTRPIPE:
    return -86;
  case EUSERS:
    return -87;
  case ENOTSOCK:
    return -88;
  case EDESTADDRREQ:
    return -89;
  case EMSGSIZE:
    return -90;
  case EPROTOTYPE:
    return -91;
  case ENOPROTOOPT:
    return -92;
  case EPROTONOSUPPORT:
    return -93;
  case ESOCKTNOSUPPORT:
    return -94;
  case EOPNOTSUPP:
    return -95;
  case EPFNOSUPPORT:
    return -96;
  case EAFNOSUPPORT:
    return -97;
  case EADDRINUSE:
    return -98;
  case EADDRNOTAVAIL:
    return -99;
  case ENETDOWN:
    return -100;
  case ENETUNREACH:
    return -101;
  case ENETRESET:
    return -102;
  case ECONNABORTED:
    return -103;
  case ECONNRESET:
    return -104;
  case ENOBUFS:
    return -105;
  case EISCONN:
    return -106;
  case ENOTCONN:
    return -107;
  case ESHUTDOWN:
    return -108;
  case ETOOMANYREFS:
    return -109;
  case ETIMEDOUT:
    return -110;
  case ECONNREFUSED:
    return -111;
  case EHOSTDOWN:
    return -112;
  case EHOSTUNREACH:
    return -113;
  case EALREADY:
    return -114;
  case EINPROGRESS:
    return -115;
  case ESTALE:
    return -116;
  case EUCLEAN:
    return -117;
  case ENOTNAM:
    return -118;
  case ENAVAIL:
    return -119;
  case EISNAM:
    return -120;
  case EREMOTEIO:
    return -121;
  case EDQUOT:
    return -122;
  case ENOMEDIUM:
    return -123;
  case EMEDIUMTYPE:
    return -124;
  case ECANCELED:
    return -125;
  case ENOKEY:
    return -126;
  case EKEYEXPIRED:
    return -127;
  case EKEYREVOKED:
    return -128;
  case EKEYREJECTED:
    return -129;
  case EOWNERDEAD:
    return -130;
  case ENOTRECOVERABLE:
    return -131;
  case ERFKILL:
    return -132;
  case EHWPOISON:
    return -133;

  default:
    return -1337;
  }
}

int rio_make_ipv4_tcp_server_socket(uint32_t ipv4_addr, uint16_t port, int *const fdp)
{
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (-1 == fd)
  {
    return errno;
  }

  int enable = 1;
  if (-1 == setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(enable)))
  {
    return errno;
  }

  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  addr.sin_addr.s_addr = ntohl(ipv4_addr);

  if (-1 == bind(fd, &addr, sizeof(addr)))
  {
    return errno;
  }

  if (-1 == listen(fd, 256))
  {
    return errno;
  }

  *fdp = fd;
  return 0;
}

struct sockaddr_in rio_sockaddr_in_test(struct sockaddr_in in) { return in; }

struct sockaddr_in rio_make_sockaddr_in(uint32_t ipv4_addr, uint16_t port)
{
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  addr.sin_addr.s_addr = ntohl(ipv4_addr);
  return addr;
}
