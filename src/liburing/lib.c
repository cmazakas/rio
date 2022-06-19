#include <liburing.h>

int rio_io_uring_opcode_supported(const struct io_uring_probe *p,
                                  int op)
{
  return io_uring_opcode_supported(p, op);
}

void rio_io_uring_prep_readv(struct io_uring_sqe *sqe, int fd,
                             const struct iovec *iovecs,
                             unsigned nr_vecs, __u64 offset)
{
  io_uring_prep_readv(sqe, fd, iovecs, nr_vecs, offset);
}

void rio_io_uring_sqe_set_data(struct io_uring_sqe *sqe, void *data)
{
  io_uring_sqe_set_data(sqe, data);
}

int rio_io_uring_wait_cqe(struct io_uring *ring,
                          struct io_uring_cqe **cqe_ptr)
{
  return io_uring_wait_cqe(ring, cqe_ptr);
}

void *rio_io_uring_cqe_get_data(const struct io_uring_cqe *cqe)
{
  return io_uring_cqe_get_data(cqe);
}
