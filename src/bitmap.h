#ifndef IMV_BITMAP_H
#define IMV_BITMAP_H

enum imv_pixelformat {
  IMV_ARGB,
  IMV_ABGR,
};

struct imv_bitmap {
  int width;
  int height;
  enum imv_pixelformat format;
  unsigned char *data;
};

/* Clean up a bitmap */
void imv_bitmap_free(struct imv_bitmap *bmp);

#endif
