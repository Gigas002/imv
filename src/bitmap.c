#include "bitmap.h"

#include <stdlib.h>

void imv_bitmap_free(struct imv_bitmap *bmp)
{
  free(bmp->data);
  free(bmp);
}
