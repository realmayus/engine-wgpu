use glam::Vec2;
use log::{debug, error, info};
use std::time::Instant;

use crate::camera::KeyState;
use crate::pipelines::{PipelineProvider, PipelineProviderKind};
use crate::{get_finalized_render_passes, get_framebuffers, RenderState, Hook};

pub fn start_renderer(
    mut state: RenderState,
    mut pipeline_providers: Vec<PipelineProviderKind>,
    mut callable: impl Hook + 'static,
) {
    info!(
        "Viewport dimensions: x={} y={}",
        state.viewport.extent[0] as u32, state.viewport.extent[1] as u32
    );
    let depth_buffer = ImageView::new_default(
        Image::new(
            state.init_state.memory_allocator.clone(),
            ImageCreateInfo {
                extent: [
                    state.viewport.extent[0] as u32,
                    state.viewport.extent[1] as u32,
                    1,
                ],
                format: Format::D16_UNORM,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .unwrap();

    let (framebuffers, mut image_views) = get_framebuffers(
        &state.init_state.images,
        &state.init_state.render_pass,
        depth_buffer,
    );

    let (camera, textures, material_infos, mesh_infos, light_info) =
        callable.get_buffers(state.init_state.device.clone());
    for provider in pipeline_providers.as_mut_slice() {
        provider.create_pipeline();
        provider.init_descriptor_sets(
            &state.init_state.descriptor_set_allocator,
            camera.clone(),
            textures.clone(),
            material_infos.clone(),
            mesh_infos.clone(),
            light_info.clone(),
        );
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
        state.init_state.image_format,
        GuiConfig {
            is_overlay: true,
            ..Default::default()
        },
    );

    let mut keys = KeyState::default();
    let mut cursor_pos = Vec2::default();
    let mut cursor_delta = Vec2::default();
    let mut delta_time = 0.01;
    let mut gui_catch = false;
    let mut previous_frame_end = Some(sync::now(state.init_state.device.clone()).boxed());
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
                        winit::event::KeyboardInput {
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
                    keys.up_pressed = is_pressed;
                }
                VirtualKeyCode::A | VirtualKeyCode::Left => {
                    keys.left_pressed = is_pressed;
                }
                VirtualKeyCode::S | VirtualKeyCode::Down => {
                    keys.down_pressed = is_pressed;
                }
                VirtualKeyCode::D | VirtualKeyCode::Right => {
                    keys.right_pressed = is_pressed;
                }
                _ => {}
            }
        }
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
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
        Event::WindowEvent {
            event:
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Middle,
                    ..
                },
            ..
        } => keys.middle_pressed = state == ElementState::Pressed,
        // todo zoom
        Event::WindowEvent {
            event: WindowEvent::ModifiersChanged(mods),
            ..
        } => keys.shift_pressed = mods.shift(),
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CursorMoved { position: pos, .. } => {
                let x = pos.x as f32 / state.viewport.extent[0];
                let y = pos.y as f32 / state.viewport.extent[1];
                cursor_delta = Vec2::new(x, y) - cursor_pos;
                cursor_pos = Vec2::new(x, y);
                gui.update(&event);
            }
            _ => {
                gui.update(&event);
            }
        },
        Event::MainEventsCleared => {
            callable.recv_input(&keys, cursor_delta, delta_time);
            cursor_delta = Vec2::default();
        }
        Event::RedrawEventsCleared => {
            let time = Instant::now();
            // TODO: Optimization: Implement Frames in Flight
            previous_frame_end.as_mut().unwrap().cleanup_finished();
            if recreate_render_passes || recreate_swapchain {
                recreate_swapchain = false;
                info!(
                    "Partial reinitialization due to {}",
                    if recreate_render_passes {
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
                        Err(e) => panic!("failed to recreate swapchain: {e}"),
                    };
                state.init_state.swapchain = new_swapchain;
                let depth_buffer = ImageView::new_default(
                    Image::new(
                        state.init_state.memory_allocator.clone(),
                        ImageCreateInfo {
                            extent: [new_dimensions.width, new_dimensions.height, 1],
                            format: Format::D16_UNORM,
                            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
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

                    state.viewport.extent = new_dimensions.into();
                    let (camera, textures, material_infos, mesh_infos, light_infos) =
                        callable.get_buffers(state.init_state.device.clone());
                    for provider in pipeline_providers.as_mut_slice() {
                        provider.create_pipeline();
                        provider.init_descriptor_sets(
                            &state.init_state.descriptor_set_allocator,
                            camera.clone(),
                            textures.clone(),
                            material_infos.clone(),
                            mesh_infos.clone(),
                            light_infos.clone(),
                        );
                        provider.set_viewport(state.viewport.clone());
                    }
                    command_buffers = get_finalized_render_passes(
                        new_framebuffers.clone(),
                        &state.init_state.cmd_buf_allocator,
                        state.init_state.queue.queue_family_index(),
                        pipeline_providers.as_mut_slice(),
                    );
                }
            }

            gui.immediate_ui(|gui| callable.setup_gui(gui));

            // acquire_next_image gives us the image index on which we are allowed to draw and a future indicating when the GPU will gain access to that image
            // suboptimal: the acquired image is still usable, but the swapchain should be recreated as the surface's properties no longer match the swapchain.
            let (image_i, suboptimal, acquire_future) =
                match swapchain::acquire_next_image(state.init_state.swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(Validated::Error(VulkanError::OutOfDate)) => {
                        recreate_swapchain = true;
                        return;
                    }
                    Err(e) => panic!("Failed to acquire next image: {e}"),
                };

            if suboptimal {
                info!("Suboptimal image encountered, recreating swapchain in next frame");
                recreate_swapchain = true;
            }

            let update_cmd_buffer = callable
                .update(
                    pipeline_providers.as_mut_slice(),
                    state.init_state.memory_allocator.clone(),
                    state.init_state.descriptor_set_allocator.clone(),
                    state.init_state.cmd_buf_allocator.clone(),
                    state.init_state.queue.queue_family_index(),
                    state.init_state.device.clone(),
                    state.viewport.clone(),
                )
                .unwrap();
            for provider in pipeline_providers.as_mut_slice() {
                recreate_render_passes =
                    recreate_render_passes || provider.must_recreate_render_passes()
            }

            let main_drawings = previous_frame_end
                .take()
                .unwrap()
                .join(acquire_future) // cmd buf can't be executed immediately, as it needs to wait for the image to actually become available
                .then_execute(
                    state.init_state.queue.clone(),
                    command_buffers[image_i as usize].clone(),
                ) // execute cmd buf which is selected based on image index
                .unwrap()
                .then_execute(state.init_state.queue.clone(), update_cmd_buffer)
                .unwrap();

            // let after_egui =
            //     gui.draw_on_image(main_drawings, image_views[image_i as usize].clone());

            // let present = after_egui
            let present = main_drawings
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
                Ok(future) => previous_frame_end = Some(future.boxed()),
                Err(Validated::Error(VulkanError::OutOfDate)) => {
                    recreate_swapchain = true;
                    previous_frame_end = Some(sync::now(state.init_state.device.clone()).boxed());
                }
                Err(e) => {
                    error!("Failed to flush future: {e}");
                    previous_frame_end = Some(sync::now(state.init_state.device.clone()).boxed());
                }
            }
            let elapsed = time.elapsed().as_micros() as f32;
            delta_time = elapsed / 1_000_000.0;
        }
        _ => {}
    });
}
