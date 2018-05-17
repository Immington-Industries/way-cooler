use compositor::{Output, Server};
use wlroots::{CompositorHandle, OutputBuilder, OutputBuilderResult, OutputManagerHandler};

pub struct OutputManager;

impl OutputManager {
    pub fn new() -> Self {
        OutputManager
    }
}

impl OutputManagerHandler for OutputManager {
    fn output_added<'output>(&mut self,
                             compositor: CompositorHandle,
                             builder: OutputBuilder<'output>)
                             -> Option<OutputBuilderResult<'output>> {
        let mut res = builder.build_best_mode(Output);
        with_handles!([(compositor: {compositor}), (output: {&mut res.output})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
            let Server { ref mut cursor,
            ref mut layout,
            ref mut xcursor_theme,
            .. } = *server;
            with_handles!([(layout: {layout}), (cursor: {cursor})] => {
                let xcursor = xcursor_theme.get_cursor("left_ptr".into())
                    .expect("Could not load left_ptr cursor");
                layout.add_auto(output);
                cursor.attach_output_layout(layout);
                cursor.set_cursor_image(&xcursor.images()[0]);
                let (x, y) = cursor.coords();
                cursor.warp(None, x, y);
            }).expect("Could not setup output with cursor and layout");
        }).unwrap();
        Some(res)
     }
}
