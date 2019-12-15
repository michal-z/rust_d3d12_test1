use crate::graphics::*;
use crate::util::*;
use glam::f32::*;
use std::ffi::CString;
use std::mem;
use std::ptr;
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::windef::HWND;
use winapi::um::d3d12::*;
use winapi::um::d3dcommon::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use winapi::um::winuser::{
    DispatchMessageA, PeekMessageA, SetProcessDPIAware, MSG, PM_REMOVE, WM_QUIT,
};

#[macro_use]
mod util;
mod dx12_wrapper;
mod graphics;

struct App {
    app_name: CString,
    dx: Dx12Context,
    frame_stats: FrameStats,
    pso: Dx12PipelineHandle,
    vertex_buffer: Dx12ResourceHandle,
}

impl App {
    fn new() -> Self {
        let app_name = CString::new("d3d12_simple").unwrap();

        let window = create_window(&app_name, 1920, 1080);
        let mut dx = Dx12Context::new(window);
        dx.reset_command_list();

        let pso = Self::create_pso(&mut dx);
        let vertex_buffer = Self::create_vertex_buffer(&mut dx);

        dx.execute_command_list();
        dx.finish();
        Self {
            app_name,
            dx,
            frame_stats: FrameStats::new(),
            pso,
            vertex_buffer,
        }
    }

    fn destroy(&mut self) {
        self.dx.finish();
        self.dx.destroy();
    }

    fn create_vertex_buffer(dx: &mut Dx12Context) -> Dx12ResourceHandle {
        let vertex_buffer = dx.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            D3D12_HEAP_FLAG_NONE,
            &Dx12ResourceDesc::buffer(1024),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );

        let (cpu_addr, buffer, offset) = dx.allocate_upload_buffer_region(127);
        let cpu_addr = cpu_addr as *mut Vec3;
        unsafe {
            *cpu_addr = Vec3::new(-0.1, -0.7, 0.0);
            *cpu_addr.offset(1) = Vec3::new(0.0, 0.7, 0.0);
            *cpu_addr.offset(2) = Vec3::new(0.7, -0.7, 0.0);
        }
        dx.cmdlist
            .copy_buffer_region(dx.get_resource(vertex_buffer), 0, buffer, offset, 48);
        dx.cmd_transition_barrier(
            vertex_buffer,
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
        );
        vertex_buffer
    }

    fn create_pso(dx: &mut Dx12Context) -> Dx12PipelineHandle {
        let semantic_position = CString::new("_Position").unwrap();
        let input = [Dx12InputElementDesc::new(
            &semantic_position,
            DXGI_FORMAT_R32G32B32_FLOAT,
            0,
        )];

        dx.create_graphics_pipeline(
            &mut D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                InputLayout: D3D12_INPUT_LAYOUT_DESC {
                    pInputElementDescs: &input as *const D3D12_INPUT_ELEMENT_DESC,
                    NumElements: input.len() as u32,
                },
                RasterizerState: Dx12RasterizerDesc::default(),
                BlendState: Dx12BlendDesc::default(),
                RTVFormats: [DXGI_FORMAT_R8G8B8A8_UNORM, 0, 0, 0, 0, 0, 0, 0],
                DepthStencilState: {
                    let mut desc = Dx12DepthStencilDesc::default();
                    desc.DepthEnable = 0;
                    desc
                },
                NumRenderTargets: 1,
                SampleMask: 0xffffffff,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                ..Default::default()
            },
            "test.vs.cso",
            "test.ps.cso",
        )
    }

    fn draw(&mut self) {
        let dx = &mut self.dx;
        let (back_buffer, back_buffer_rtv) = dx.get_back_buffer();
        dx.reset_command_list();
        dx.cmdlist.rs_set_viewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: dx.resolution[0] as f32,
            Height: dx.resolution[1] as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        dx.cmdlist.rs_set_scissor_rects(&[D3D12_RECT {
            top: 0,
            left: 0,
            right: dx.resolution[0] as i32,
            bottom: dx.resolution[1] as i32,
        }]);
        dx.cmd_transition_barrier(back_buffer, D3D12_RESOURCE_STATE_RENDER_TARGET);
        dx.cmdlist.om_set_render_target(back_buffer_rtv, None);
        dx.cmdlist.clear_render_target_view(
            back_buffer_rtv,
            &[0.2 as f32, 0.4 as f32, 0.8 as f32, 1.0 as f32],
            &[],
        );
        dx.cmd_set_graphics_pipeline(self.pso);
        dx.cmdlist
            .ia_set_primitive_topology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        dx.cmdlist.ia_set_vertex_buffers(
            0,
            &[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: dx
                    .get_resource(self.vertex_buffer)
                    .get_gpu_virtual_address(),
                SizeInBytes: 48,
                StrideInBytes: 16,
            }],
        );
        dx.cmdlist.draw_instanced(3, 1, 0, 0);
        dx.cmd_transition_barrier(back_buffer, D3D12_RESOURCE_STATE_PRESENT);
        dx.execute_command_list();
        dx.present_frame(0);
    }

    fn run(&mut self) {
        let window = self.dx.window;

        'mainloop: loop {
            unsafe {
                let mut message: MSG = mem::zeroed();
                while PeekMessageA(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    DispatchMessageA(&message);
                    if message.message == WM_QUIT {
                        break 'mainloop;
                    }
                }
            }

            self.frame_stats.update(window, &self.app_name);
            self.draw();
        }
        self.destroy();
    }
}

fn main() {
    unsafe { SetProcessDPIAware() };
    App::new().run();
}
