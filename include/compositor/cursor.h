#ifndef WC_CURSOR_H
#define WC_CURSOR_H

#include <wayland-server.h>
#include <wlr/types/wlr_box.h>
#include <wlr/types/wlr_cursor.h>
#include <wlr/types/wlr_seat.h>

enum wc_cursor_mode {
	WC_CURSOR_PASSTHROUGH = 0,
	WC_CURSOR_MOVE,
	WC_CURSOR_RESIZE,
};

struct wc_cursor {
	struct wc_server *server;
	struct wlr_cursor *wlr_cursor;

	// When non-NULL, this takes precedence over all other cursor images.
	char *compositor_image;
	// Flag to determine if we are using the client provided image.
	bool use_client_image;
	/* The image to use if there is no compositor_image or use_client_image is
	 * not set.
	 */
	const char *default_image;

	enum wc_cursor_mode cursor_mode;
	/*
	 * Original location data of a view when it is grabbed. This is
	 * used in calculations when resizing and moving it from
	 * the original location.
	 *
	 * depending on mode, these may or may not be valid
	 */
	struct {
		struct wc_view *view;
		// Original coordinates of where the cursor was.
		int original_x, original_y;
		struct wlr_box original_view_geo;
		uint32_t resize_edges;
	} grabbed;

	struct wl_listener motion;
	struct wl_listener motion_absolute;
	struct wl_listener button;
	struct wl_listener axis;
	struct wl_listener frame;
};

void wc_cursor_init(struct wc_server *server);

void wc_cursor_fini(struct wc_server *server);

/* Sets the cursor image, given from the client. If cursor_name is NULL then
 * it will defer to the default compositor cursor.
 *
 * If the compositor cursor is set to a non-NULL value then this value will be
 * ignored until that is null. Once the compositor cursor becomes non-NULL
 * there is no need to recall this, it will be done automatically.
 *
 * To switch to using the compositor cursor again, use wc_set_compositor_cursor.
 */
void wc_cursor_set_client_cursor(struct wc_cursor *cursor,
		struct wlr_seat_pointer_request_set_cursor_event *event);

/* Sets the current image used by the compositor. If cursor_name is NULL then
 * it will set the cursor to the client provided cursor, or to the default
 * cursor if there is no current client provided one.
 *
 * This is primarily used by mousegrabber, and is only intended to allow the
 * special Awesome client to change the cursor. This is basically a huge
 * subversion of how Wayland is supposed to work.
 */
void wc_cursor_set_compositor_cursor(
		struct wc_cursor *cursor, const char *cursor_name);

#endif  // WC_CURSOR_H
