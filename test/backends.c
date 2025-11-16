#include "backend.h"
#include "backends.h"
#include "source.h"

#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#include <cmocka.h>

static int setup(void **state)
{
  struct backends *answer = backends_create();
  if (!answer) {
    return -1;
  }
  *state = answer;
  return 0;
}

static int teardown(void **state)
{
  backends_free(*state);
  return 0;
}

static void test_open_garbage_fails(void **state)
{
  char data[] = {1, 2, 3, 4};
  struct imv_source *src;
  enum backend_result res = backends_open_memory(*state, &data, sizeof(data), &src);
  assert_int_equal(res, BACKEND_UNSUPPORTED);
}

int main(void)
{
  const struct CMUnitTest tests[] = {
      cmocka_unit_test(test_open_garbage_fails),
  };

  return cmocka_run_group_tests(tests, setup, teardown);
}
