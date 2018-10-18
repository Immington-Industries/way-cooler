use wlroots::{CompositorHandle, OutputBuilder, OutputBuilderResult, OutputManagerHandler};

pub struct OutputManager;

impl OutputManager {
    pub fn new() -> Self {
        OutputManager
    }
}

impl OutputManagerHandler for OutputManager {
    fn output_added<'output>(
        &mut self,
        compositor: CompositorHandle,
        builder: OutputBuilder<'output>,
    ) -> Option<OutputBuilderResult<'output>> {
        dehandle!(
            @compositor = {compositor};
            let server: &mut ::Server = compositor.into();
            let res = builder.build_best_mode(::Output);
            server.outputs.push(res.output.clone());
            let ::Server { ref mut cursor,
                         ref mut layout,
                         ref mut xcursor_manager,
                         .. } = *server;
            @layout = {layout};
            @cursor = {cursor};
            {
                @output = {&res.output};
                layout.add_auto(output);
                cursor.attach_output_layout(layout);
                xcursor_manager.load(output.scale());
                xcursor_manager.set_cursor_image("left_ptr".to_string(), cursor);
                let (x, y) = cursor.coords();
                cursor.warp(None, x, y)
            }
            Some(res)
        )
    }
}
