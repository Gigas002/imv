#ifndef IMV_BITMAP_H
#define IMV_BITMAP_H

struct imv_bitmap {
  int width;
  int height;
  unsigned char *data;
};

/* Clean up a bitmap */
void imv_bitmap_free(struct imv_bitmap bmp);

#endif
