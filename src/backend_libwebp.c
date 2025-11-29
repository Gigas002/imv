#include "backend.h"
#include "bitmap.h"
#include "image.h"
#include "log.h"
#include "source.h"
#include "source_private.h"

#include <stdbool.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <webp/decode.h>

struct private {
  void *data;
  ssize_t data_len;
  bool is_mmaped;
};

static void free_private(void *raw_pvt)
{
  if (raw_pvt == NULL) {
    return;
  }

  struct private *pvt = raw_pvt;

  if (pvt->is_mmaped) {
    munmap(pvt->data, pvt->data_len);
    pvt->is_mmaped = false;
  }

  free(pvt);
}

static void first_frame(void *raw_pvt, struct imv_image **img, int *frametime)
{
  *img = NULL;
  *frametime = 0;

  imv_log(IMV_DEBUG, "libwebp: first_frame called\n");

  struct private *pvt = raw_pvt;

  struct imv_bitmap bmp;

  bmp.data = WebPDecodeRGBA(pvt->data, pvt->data_len, &bmp.width, &bmp.height);
  if (bmp.data == NULL) {
    imv_log(IMV_ERROR, "libwebp: failed to decode image\n");
    return;
  }

  *img = imv_image_create_from_bitmap(bmp);
}

static const struct imv_source_vtable vtable = {
  .load_first_frame = first_frame,
  .free = free_private,
};

static enum backend_result open_memory_internal(struct private priv,
                                                struct imv_source **src)
{
  if (priv.data == NULL) {
    return BACKEND_BAD_PATH;
  }

  if (WebPGetInfo(priv.data, priv.data_len, NULL, NULL) != true) {
    imv_log(IMV_DEBUG, "libwebp: error interpreting file header as webp\n");
    return BACKEND_UNSUPPORTED;
  }

  struct private *pvt = calloc(1, sizeof *pvt);
  *pvt = priv;

  *src = imv_source_create(&vtable, pvt);

  return BACKEND_SUCCESS;
}

static enum backend_result open_memory(void *data, size_t data_len, struct imv_source **src)
{
  imv_log(IMV_DEBUG, "libwebp: open_memory called\n");
  return open_memory_internal((struct private){data, data_len, false }, src);
}

static enum backend_result open_path(const char *path, struct imv_source **src)
{
  imv_log(IMV_DEBUG, "libwebp: open_path(%s)\n", path);

  struct private priv = {0};

  int fd = open(path, O_RDONLY);
  if (fd < 0) {
    goto close;
  }

  long data_len = lseek(fd, 0, SEEK_END);
  if (data_len < 0) {
    goto close;
  }
  void *data = mmap(NULL, data_len, PROT_READ, MAP_PRIVATE, fd, 0);
  if (data == NULL || data == MAP_FAILED) {
    imv_log(IMV_ERROR, "libwebp: failed to map file into memory\n");
    goto close;
  }
  priv.data = data;
  priv.data_len = data_len;
  priv.is_mmaped = true;

close:
  if (fd >= 0)
    close(fd);

  return open_memory_internal(priv, src);
}

const struct imv_backend imv_backend_libwebp = {
  .name = "libwebp",
  .description = "The official WebP implementation.",
  .website = "https://developers.google.com/speed/webp",
  .license = "The Modified BSD License",
  .open_path = &open_path,
  .open_memory = &open_memory,
};
