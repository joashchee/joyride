/* Drives libogg and libtheora's decoder for ogv: feed bytes in, get RGBA
   frames out. Written for JoyrIDE (MIT). The Ogg container is also parsed in
   Rust (src/lib.rs) for probing; this decodes. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <ogg/ogg.h>
#include <theora/theoradec.h>

typedef struct {
  ogg_sync_state sync;
  ogg_stream_state vs;
  int have_stream;        /* the Theora stream has been picked */
  long serial;
  th_info ti;
  th_comment tc;
  th_setup_info *ts;
  th_dec_ctx *dec;
  int headers_done;
  int pending;            /* packets remain in vs */
  uint8_t *rgba;
  size_t rgba_len;
  double time;            /* of the last frame */
  int64_t index;          /* of the last frame */
  int eof_fed;
} jr_ogv;

jr_ogv *jr_ogv_new(void) {
  jr_ogv *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  ogg_sync_init(&h->sync);
  th_info_init(&h->ti);
  th_comment_init(&h->tc);
  return h;
}

void jr_ogv_free(jr_ogv *h) {
  if (!h) return;
  if (h->dec) th_decode_free(h->dec);
  if (h->ts) th_setup_free(h->ts);
  if (h->have_stream) ogg_stream_clear(&h->vs);
  th_comment_clear(&h->tc);
  th_info_clear(&h->ti);
  ogg_sync_clear(&h->sync);
  free(h->rgba);
  free(h);
}

int jr_ogv_feed(jr_ogv *h, const uint8_t *data, size_t n) {
  char *buf = ogg_sync_buffer(&h->sync, (long)n);
  if (!buf) return -1;
  memcpy(buf, data, n);
  return ogg_sync_wrote(&h->sync, (long)n);
}

static uint8_t clamp(int v) { return v < 0 ? 0 : v > 255 ? 255 : (uint8_t)v; }

/* BT.601 limited range, as Theora's own colour spaces use for these files. */
static int convert(jr_ogv *h) {
  th_ycbcr_buffer yb;
  if (th_decode_ycbcr_out(h->dec, yb)) return -1;
  int w = (int)h->ti.pic_width, ht = (int)h->ti.pic_height;
  int px = (int)h->ti.pic_x, py = (int)h->ti.pic_y;
  int xd = h->ti.pixel_fmt != TH_PF_444, yd = h->ti.pixel_fmt == TH_PF_420;
  size_t need = (size_t)w * ht * 4;
  if (h->rgba_len != need) {
    uint8_t *p = realloc(h->rgba, need);
    if (!p) return -1;
    h->rgba = p; h->rgba_len = need;
  }
  for (int y = 0; y < ht; y++) {
    const uint8_t *yr = yb[0].data + (size_t)(py + y) * yb[0].stride + px;
    const uint8_t *cb = yb[1].data + (size_t)((py + y) >> yd) * yb[1].stride;
    const uint8_t *cr = yb[2].data + (size_t)((py + y) >> yd) * yb[2].stride;
    uint8_t *o = h->rgba + (size_t)y * w * 4;
    for (int x = 0; x < w; x++) {
      int c = ((int)yr[x] - 16) * 298, d = (int)cb[(px + x) >> xd] - 128, e = (int)cr[(px + x) >> xd] - 128;
      o[x * 4 + 0] = clamp((c + 409 * e + 128) >> 8);
      o[x * 4 + 1] = clamp((c - 100 * d - 208 * e + 128) >> 8);
      o[x * 4 + 2] = clamp((c + 516 * d + 128) >> 8);
      o[x * 4 + 3] = 255;
    }
  }
  return 0;
}

/* Handles one packet of the Theora stream: 0 nothing to show, 1 a frame is
   ready, <0 an error. */
static int packet(jr_ogv *h, ogg_packet *op) {
  if (!h->headers_done) {
    int r = th_decode_headerin(&h->ti, &h->tc, &h->ts, op);
    if (r > 0) return 0;
    if (r < 0) return -2;
    /* 0: this is the first video data packet, after the headers; decode it. */
    h->dec = th_decode_alloc(&h->ti, h->ts);
    if (!h->dec) return -3;
    h->headers_done = 1;
  }
  ogg_int64_t gp = -1;
  int r = th_decode_packetin(h->dec, op, &gp);
  if (r == TH_DUPFRAME) { /* repeat the last picture */
    h->index++;
    h->time = (double)h->index * h->ti.fps_denominator / h->ti.fps_numerator;
    return h->rgba ? 1 : 0;
  }
  if (r != 0) return -4;
  h->index = gp >= 0 ? th_granule_frame(h->dec, gp) : h->index + 1;
  h->time = (double)h->index * h->ti.fps_denominator / h->ti.fps_numerator; /* when it starts */
  return convert(h) ? -5 : 1;
}

/* Advance to the next frame. 1 a frame is ready, 0 more bytes are needed
   (jr_ogv_feed, then call again), 2 the stream ended cleanly, <0 an error. */
int jr_ogv_next(jr_ogv *h) {
  for (;;) {
    ogg_packet op;
    while (h->have_stream && ogg_stream_packetout(&h->vs, &op) > 0) {
      int r = packet(h, &op);
      if (r) return r;
    }
    ogg_page og;
    int g = ogg_sync_pageout(&h->sync, &og);
    if (g == 0) return h->eof_fed ? 2 : 0;
    if (g < 0) continue; /* skipped bytes: resync */
    if (ogg_page_bos(&og)) {
      ogg_stream_state t;
      ogg_stream_init(&t, ogg_page_serialno(&og));
      ogg_packet first;
      int is_theora = 0;
      if (ogg_stream_pagein(&t, &og) == 0 && ogg_stream_packetpeek(&t, &first) > 0) {
        th_info ti; th_comment tc; th_setup_info *ts = NULL;
        th_info_init(&ti); th_comment_init(&tc);
        is_theora = !h->have_stream && th_decode_headerin(&ti, &tc, &ts, &first) >= 0;
        th_setup_free(ts); th_comment_clear(&tc); th_info_clear(&ti);
      }
      if (is_theora) {
        h->vs = t; h->have_stream = 1; h->serial = ogg_page_serialno(&og);
      } else {
        ogg_stream_clear(&t);
      }
      continue;
    }
    if (h->have_stream && ogg_page_serialno(&og) == h->serial) ogg_stream_pagein(&h->vs, &og);
  }
}

void jr_ogv_set_eof(jr_ogv *h) { h->eof_fed = 1; }
int jr_ogv_headers_done(const jr_ogv *h) { return h->headers_done; }
uint32_t jr_ogv_width(const jr_ogv *h) { return h->ti.pic_width; }
uint32_t jr_ogv_height(const jr_ogv *h) { return h->ti.pic_height; }
const uint8_t *jr_ogv_rgba(const jr_ogv *h) { return h->rgba; }
size_t jr_ogv_rgba_len(const jr_ogv *h) { return h->rgba_len; }
double jr_ogv_time(const jr_ogv *h) { return h->time; }
int64_t jr_ogv_index(const jr_ogv *h) { return h->index; }
