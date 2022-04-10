#include <liburing.h>

int rio_io_uring_opcode_supported(const struct io_uring_probe *p,
                                  int op)
{
  return io_uring_opcode_supported(p, op);
}