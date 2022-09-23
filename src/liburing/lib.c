#include <liburing.h>

#include <sys/socket.h>
#include <sys/un.h>
#include <sys/timerfd.h>

#include <assert.h>
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

void rio_io_uring_prep_accept_af_unix(struct io_uring_sqe *sqe, int fd)
{
  struct sockaddr_un sockaddr;
  memset(&sockaddr, 0, sizeof(sockaddr));
  socklen_t addrlen = 0;
  io_uring_prep_accept(sqe, fd, (struct sockaddr *)&sockaddr, &addrlen, 0);
}

void rio_io_uring_prep_read(struct io_uring_sqe *sqe,
                            int fd, void *buf, unsigned nbytes, off_t offset)
{
  return io_uring_prep_read(sqe, fd, buf, nbytes, offset);
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

void rio_io_uring_cqe_seen(struct io_uring *ring, struct io_uring_cqe *cqe)
{
  io_uring_cqe_seen(ring, cqe);
}

int rio_timerfd_create()
{
  int fd = timerfd_create(CLOCK_REALTIME, 0);
  return fd;
}

int rio_timerfd_settime(int fd, int millis)
{
  struct timespec now;
  if (-1 == clock_gettime(CLOCK_REALTIME, &now))
  {
    return -1;
  }

  struct itimerspec expiry;
  expiry.it_value.tv_sec = now.tv_sec + (millis / 1000);
  expiry.it_value.tv_nsec = now.tv_nsec + (1000 * (millis % 1000));
  expiry.it_interval.tv_sec = 0;
  expiry.it_interval.tv_nsec = 0;

  if (-1 == timerfd_settime(fd, TFD_TIMER_ABSTIME, &expiry, NULL))
  {
    return -1;
  }

  return 0;
}
