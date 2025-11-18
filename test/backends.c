#include "backend.h"
#include "backends.h"
#include "image.h"
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
  enum backend_result res =
      backends_open_memory(*state, &data, sizeof(data), &src);
  assert_int_equal(res, BACKEND_UNSUPPORTED);
}

static const uint8_t SAMPLE_WIDTH = 3;
static const uint8_t SAMPLE_HEIGHT = 3;
static const uint8_t SAMPLE_RAW[] = {0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00,
    0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0xff, 0x88, 0x88, 0x88,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0xff, 0xff, 0x00, 0xff,
    0xff, 0x00, 0xff, 0xff, 0xff};

static void assert_bitmap_equal_to_sample(struct imv_bitmap *bitmap)
{
  assert_int_equal(bitmap->format, IMV_ABGR);
  assert_int_equal(bitmap->width, SAMPLE_WIDTH);
  assert_int_equal(bitmap->height, SAMPLE_HEIGHT);
  assert_memory_equal(bitmap->data, &SAMPLE_RAW, sizeof(SAMPLE_RAW));
}

#ifdef IMV_BACKEND_LIBTIFF
extern unsigned char sample_tiff[];
extern unsigned int sample_tiff_len;
static void test_open_ttf_file(void **state)
{
  struct imv_source *src;
  assert_int_equal(
      backends_open_memory(*state, sample_tiff, sample_tiff_len, &src),
      BACKEND_SUCCESS);

  struct imv_image *image;
  int frametime;
  assert_true(imv_source_load_first_frame(src, &image, &frametime));

  assert_int_equal(frametime, 0);
  struct imv_bitmap *bitmap = imv_image_get_bitmap(image);
  assert_non_null(bitmap);
  assert_bitmap_equal_to_sample(bitmap);

  imv_image_free(image);
  imv_source_free(src);
}
#endif

int main(void)
{
  const struct CMUnitTest tests[] = {
      cmocka_unit_test(test_open_garbage_fails),
#ifdef IMV_BACKEND_LIBTIFF
      cmocka_unit_test(test_open_ttf_file),
#endif
  };

  return cmocka_run_group_tests(tests, setup, teardown);
}
