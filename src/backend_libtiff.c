#include "backend.h"
#include "bitmap.h"
#include "image.h"
#include "source.h"
#include "source_private.h"

#include <stdlib.h>
#include <string.h>
#include <tiffio.h>

struct private {
  TIFF *tiff;
  int width;
  int height;
};

static tsize_t mem_read(thandle_t data, tdata_t buffer, tsize_t len)
{
  return fread(buffer, 1, len, data);
}

static tsize_t mem_write(thandle_t data, tdata_t buffer, tsize_t len)
{
  return fwrite(buffer, 1, len, data);
}

static int mem_close(thandle_t data) { return fclose(data); }

static toff_t mem_seek(thandle_t data, toff_t pos, int whence)
{
  return fseek(data, pos, whence) == -1 ? -1 : ftell(data);
}

static toff_t mem_size(thandle_t data)
{
  long pos = ftell(data);
  if (fseek(data, 0L, SEEK_END) != 0) {
    return -1;
  }
  long size = ftell(data);
  if (fseek(data, pos, SEEK_SET) != 0) {
    return -1;
  }
  return size;
}

static void free_private(void *raw_private)
{
  if (!raw_private) {
    return;
  }

  struct private *private = raw_private;
  TIFFClose(private->tiff);
  private->tiff = NULL;

  free(private);
}

static void load_image(void *raw_private, struct imv_image **image, int *frametime)
{
  *image = NULL;
  *frametime = 0;

  struct private *private = raw_private;

  /* libtiff suggests using their own allocation routines to support systems
   * with segmented memory. I have no desire to support that, so I'm just
   * going to use vanilla malloc/free. Systems where that isn't acceptable
   * don't have upstream support from imv.
   */
  void *bitmap = malloc(private->height * private->width * 4);
  int rcode = TIFFReadRGBAImageOriented(private->tiff, private->width, private->height,
      bitmap, ORIENTATION_TOPLEFT, 0);

  /* 1 = success, unlike the rest of *nix */
  if (rcode != 1) {
    return;
  }

  struct imv_bitmap *bmp = malloc(sizeof *bmp);
  bmp->width = private->width;
  bmp->height = private->height;
  bmp->format = IMV_ABGR;
  bmp->data = bitmap;
  *image = imv_image_create_from_bitmap(bmp);
}

static const struct imv_source_vtable vtable = {
  .load_first_frame = load_image,
  .free = free_private
};

static enum backend_result open_path(const char *path, struct imv_source **src)
{
  struct private private;

  TIFFSetErrorHandler(NULL);

  private.tiff = TIFFOpen(path, "r");
  if (!private.tiff) {
    /* Header is read, so no BAD_PATH check here */
    return BACKEND_UNSUPPORTED;
  }

  TIFFGetField(private.tiff, TIFFTAG_IMAGEWIDTH, &private.width);
  TIFFGetField(private.tiff, TIFFTAG_IMAGELENGTH, &private.height);

  struct private *new_private = malloc(sizeof private);
  memcpy(new_private, &private, sizeof private);

  *src = imv_source_create(&vtable, new_private);
  return BACKEND_SUCCESS;
}

static enum backend_result open_memory(void *data, size_t len, struct imv_source **src)
{
  TIFFSetErrorHandler(NULL);
  FILE *f = fmemopen(data, len, "r");
  if (!f) {
    return BACKEND_ERROR;
  }
  struct private *priv = malloc(sizeof *priv);
  if (!priv) {
    return BACKEND_ERROR;
  }
  priv->tiff = TIFFClientOpen("-", "rm", (thandle_t)f, &mem_read, &mem_write, &mem_seek,
      &mem_close, &mem_size, NULL, NULL);
  if (!priv->tiff) {
    /* Header is read, so no BAD_PATH check here */
    free(priv);
    return BACKEND_UNSUPPORTED;
  }

  TIFFGetField(priv->tiff, TIFFTAG_IMAGEWIDTH, &priv->width);
  TIFFGetField(priv->tiff, TIFFTAG_IMAGELENGTH, &priv->height);

  *src = imv_source_create(&vtable, priv);
  return BACKEND_SUCCESS;
}

const struct imv_backend imv_backend_libtiff = {
  .name = "libtiff",
  .description = "The de-facto tiff library",
  .website = "http://www.libtiff.org/",
  .license = "MIT",
  .open_path = &open_path,
  .open_memory = &open_memory,
};
