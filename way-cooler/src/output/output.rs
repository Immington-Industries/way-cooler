use std::rc::Rc;

use wlroots::utils::current_time;
use wlroots::{
    project_box, Area, CompositorHandle, Origin, OutputHandle, OutputHandler, OutputLayoutHandle,
    Renderer, Size, SurfaceHandle,
};

use Server;

pub struct Output;

impl OutputHandler for Output {
    fn on_frame(&mut self, compositor: CompositorHandle, output: OutputHandle) {
        dehandle!(
            @compositor = {compositor};
            @output = {output};
            let state: &mut Server = compositor.data.downcast_mut().unwrap();
            let Server { ref mut layout,
            ref mut views,
            .. } = *state;
            let renderer = compositor.renderer.as_mut().expect("gles2 disabled");
            let mut renderer = renderer.render(output, None);
            renderer.clear([0.25, 0.25, 0.25, 1.0]);
            render_views(&mut renderer, layout, views)
            )
    }
}

fn render_surface(
    renderer: &mut Renderer,
    layout: &mut OutputLayoutHandle,
    surface: &mut SurfaceHandle,
    lx: i32,
    ly: i32,
) {
    with_handles!([(surface: {surface}), (layout: {&mut *layout})] => {
        let (width, height) = surface.current_state().size();
        let (render_width, render_height) =
            (width * renderer.output.scale() as i32,
            height * renderer.output.scale() as i32);
        let render_box = Area::new(Origin::new(lx, ly),
        Size::new(render_width,
                  render_height));

        if layout.intersects(renderer.output, render_box) {
            let transform = renderer.output.get_transform().invert();
            let matrix = project_box(render_box,
                                     transform,
                                     0.0,
                                     renderer.output
                                     .transform_matrix());
            renderer.render_texture_with_matrix(&surface.texture(), matrix);
            surface.send_frame_done(current_time());
        }

    }).unwrap();
}

/// Render all of the client views.
fn render_views(
    renderer: &mut Renderer,
    layout: &mut OutputLayoutHandle,
    views: &mut Vec<Rc<::View>>,
) {
    for view in views.iter_mut().rev() {
        let origin = view.origin.get();
        view.for_each_surface(&mut |surface: SurfaceHandle, sx, sy| {
            dehandle!(
                @surface = {surface};
                @layout = {&*layout};
                let (width, height) = surface.current_state().size();
                let (render_width, render_height) =
                    (width * renderer.output.scale() as i32,
                     height * renderer.output.scale() as i32);
                let render_box = Area::new(Origin::new(origin.x + sx, origin.y + sy),
                                           Size::new(render_width,
                                                     render_height));

                if layout.intersects(renderer.output, render_box) {
                    let transform = renderer.output.get_transform().invert();
                    let matrix = project_box(render_box,
                                             transform,
                                             0.0,
                                             renderer.output
                                             .transform_matrix());
                    renderer.render_texture_with_matrix(&surface.texture(), matrix);
                    surface.send_frame_done(current_time());
                })
        });
    }
}
