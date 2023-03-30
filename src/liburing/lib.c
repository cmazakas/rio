#include <liburing.h>

#include <sys/socket.h>
#include <sys/un.h>
#include <sys/timerfd.h>
#include <sys/time.h>

#include <netinet/in.h>
#include <netinet/ip.h>
#include <netinet/tcp.h>

#include <arpa/inet.h>

#include <linux/time_types.h>

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

int rio_make_pipe(int pipefd[2])
{
  return pipe(pipefd);
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

int rio_make_ipv6_tcp_server_socket(struct in6_addr ipv6_addr, uint16_t port, int *const fdp)
{
  int fd = socket(AF_INET6, SOCK_STREAM, 0);
  if (-1 == fd)
  {
    return errno;
  }

  int enable = 1;
  if (-1 == setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(enable)))
  {
    return errno;
  }

  struct sockaddr_in6 addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin6_addr = ipv6_addr;
  addr.sin6_port = htons(port);
  addr.sin6_family = AF_INET6;

  if (-1 == bind(fd, (struct sockaddr *)&addr, sizeof(addr)))
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

struct __kernel_timespec rio_timespec_test(struct __kernel_timespec in) { return in; }
