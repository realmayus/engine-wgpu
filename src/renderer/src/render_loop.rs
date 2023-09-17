use egui_winit_vulkano::{Gui, GuiConfig};
use log::{debug, error, info};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::AttachmentImage;
use vulkano::swapchain::{
    AcquireError, SwapchainCreateInfo, SwapchainCreationError, SwapchainPresentInfo,
};
use vulkano::sync::{FlushError, GpuFuture};
use vulkano::{swapchain, sync};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

use crate::pipelines::{PipelineProvider, PipelineProviderKind};
use crate::{
    get_finalized_render_passes, get_framebuffers, PartialRenderState, RenderState, StateCallable,
};

pub fn start_renderer(
    mut state: RenderState,
    mut pipeline_providers: Vec<PipelineProviderKind>,
    mut callable: impl StateCallable + 'static,
) {
    info!(
        "Viewport dimensions: x={} y={}",
        state.viewport.dimensions[0] as u32, state.viewport.dimensions[1] as u32
    );
    let depth_buffer = ImageView::new_default(
        AttachmentImage::transient(
            &state.init_state.memory_allocator,
            [
                state.viewport.dimensions[0] as u32,
                state.viewport.dimensions[1] as u32,
            ],
            Format::D16_UNORM,
        )
        .unwrap(),
    )
    .unwrap();

    let (framebuffers, mut image_views) = get_framebuffers(
        &state.init_state.images,
        &state.init_state.render_pass,
        depth_buffer,
    );

    for provider in pipeline_providers.as_mut_slice() {
        provider.create_pipeline();
        provider.init_descriptor_sets(&state.init_state.descriptor_set_allocator);
    }
    let mut command_buffers = get_finalized_render_passes(
        framebuffers,
        &state.init_state.cmd_buf_allocator,
        state.init_state.queue.queue_family_index(),
        pipeline_providers.as_mut_slice(),
    );

    let mut recreate_render_passes = false;
    let mut recreate_swapchain = false;

    let cmd_buf = state.cmd_buf_builder.build().unwrap();

    let future = sync::now(state.init_state.device.clone())
        .then_execute(state.init_state.queue.clone(), cmd_buf)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let mut gui = Gui::new(
        &state.init_state.event_loop,
        state.init_state.surface,
        state.init_state.queue.clone(),
        GuiConfig {
            is_overlay: true,
            ..Default::default()
        },
    );

    let mut is_left_pressed = false;
    let mut is_right_pressed = false;
    let mut is_up_pressed = false;
    let mut is_down_pressed = false;
    let mut gui_catch = false;

    let event_loop = state.init_state.event_loop;

    // blocks main thread forever and calls closure whenever the event loop receives an event
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            callable.cleanup();
            *control_flow = ControlFlow::Exit;
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } => {
            recreate_render_passes = true;
        }
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                },
            ..
        } if !gui_catch && keycode != VirtualKeyCode::Escape => {
            let is_pressed = state == ElementState::Pressed;
            match keycode {
                VirtualKeyCode::W | VirtualKeyCode::Up => {
                    is_up_pressed = is_pressed;
                }
                VirtualKeyCode::A | VirtualKeyCode::Left => {
                    is_left_pressed = is_pressed;
                }
                VirtualKeyCode::S | VirtualKeyCode::Down => {
                    is_down_pressed = is_pressed;
                }
                VirtualKeyCode::D | VirtualKeyCode::Right => {
                    is_right_pressed = is_pressed;
                }
                _ => {}
            }
        }
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: key_state,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                },
            ..
        } if keycode == VirtualKeyCode::Escape => {
            if key_state == ElementState::Released {
                gui_catch = !gui_catch;
                if gui_catch {
                    state.init_state.window.set_title("Engine Playground");
                } else {
                    state
                        .init_state
                        .window
                        .set_title("Engine Playground - Press ESC to release controls");
                }
                debug!(
                    "Gui catch is now: {}",
                    if gui_catch { "enabled" } else { "disabled" }
                );
            }
        }
        Event::WindowEvent { event, .. } => {
            gui.update(&event);
        }
        Event::MainEventsCleared => {
            state.camera.recv_input(
                is_up_pressed,
                is_down_pressed,
                is_left_pressed,
                is_right_pressed,
            );
        }
        Event::RedrawEventsCleared => {
            // TODO: Optimization: Implement Frames in Flight
            if recreate_render_passes || recreate_swapchain {
                recreate_swapchain = false;
                info!(
                    "Partial reinitialization due to {}",
                    if (recreate_render_passes) {
                        "window resize"
                    } else {
                        "request to recreate swapchain"
                    }
                );
                let new_dimensions = state.init_state.window.inner_size();

                let (new_swapchain, new_images) =
                    match state.init_state.swapchain.recreate(SwapchainCreateInfo {
                        image_extent: new_dimensions.into(),
                        ..state.init_state.swapchain.create_info()
                    }) {
                        Ok(r) => r,
                        Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                        Err(e) => panic!("failed to recreate swapchain: {e}"),
                    };
                state.init_state.swapchain = new_swapchain;
                let depth_buffer = ImageView::new_default(
                    AttachmentImage::transient(
                        &state.init_state.memory_allocator,
                        new_dimensions.into(),
                        Format::D16_UNORM,
                    )
                    .unwrap(),
                )
                .unwrap();
                let (new_framebuffers, new_image_views) = get_framebuffers(
                    &new_images,
                    &state.init_state.render_pass,
                    depth_buffer.clone(),
                );
                image_views = new_image_views;
                if recreate_render_passes {
                    recreate_render_passes = false;

                    state.viewport.dimensions = new_dimensions.into();
                    for provider in pipeline_providers.as_mut_slice() {
                        provider.create_pipeline();
                        provider.init_descriptor_sets(&state.init_state.descriptor_set_allocator);
                        provider.set_viewport(state.viewport.clone());
                    }
                    command_buffers = get_finalized_render_passes(
                        new_framebuffers.clone(),
                        &state.init_state.cmd_buf_allocator,
                        state.init_state.queue.queue_family_index(),
                        pipeline_providers.as_mut_slice(),
                    );
                    state
                        .camera
                        .update_aspect(state.viewport.dimensions[0], state.viewport.dimensions[1]);
                }
            }

            gui.immediate_ui(|gui| {
                callable.setup_gui(
                    gui,
                    PartialRenderState {
                        camera: &mut state.camera,
                    },
                )
            });

            // acquire_next_image gives us the image index on which we are allowed to draw and a future indicating when the GPU will gain access to that image
            // suboptimal: the acquired image is still usable, but the swapchain should be recreated as the surface's properties no longer match the swapchain.
            let (image_i, suboptimal, acquire_future) =
                match swapchain::acquire_next_image(state.init_state.swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(AcquireError::OutOfDate) => {
                        recreate_swapchain = true;
                        return;
                    }
                    Err(e) => panic!("Failed to acquire next image: {e}"),
                };
            if suboptimal {
                info!("Suboptimal image encountered, recreating swapchain in next frame");
                recreate_swapchain = true;
            }
            acquire_future.wait(None).unwrap();
            state.camera.update_view(); // TODO optimization: only update camera uniform if dirty
            let update_cmd_buffer = callable.update(
                pipeline_providers.as_mut_slice(),
                &state.init_state.memory_allocator,
                &state.init_state.descriptor_set_allocator,
                &state.init_state.cmd_buf_allocator,
                state.init_state.queue.queue_family_index(),
            );
            for provider in pipeline_providers.as_mut_slice() {
                recreate_render_passes =
                    recreate_render_passes || provider.must_recreate_render_passes()
            }

            let main_drawings = sync::now(state.init_state.device.clone())
                .join(acquire_future) // cmd buf can't be executed immediately, as it needs to wait for the image to actually become available
                .then_execute(
                    state.init_state.queue.clone(),
                    command_buffers[image_i as usize].clone(),
                ) // execute cmd buf which is selected based on image index
                .unwrap()
                .then_execute(state.init_state.queue.clone(), update_cmd_buffer.unwrap())
                .unwrap();

            let after_egui =
                gui.draw_on_image(main_drawings, image_views[image_i as usize].clone());

            let present = after_egui
                .then_swapchain_present(
                    // tell the swapchain that we finished drawing and the image is ready for display
                    state.init_state.queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(
                        state.init_state.swapchain.clone(),
                        image_i,
                    ),
                )
                .then_signal_fence_and_flush();

            match present {
                Ok(future) => future.wait(None).unwrap(),
                Err(FlushError::OutOfDate) => {
                    recreate_swapchain = true;
                }
                Err(e) => {
                    error!("Failed to flush future: {e}");
                }
            }
        }
        _ => {}
    });
}
