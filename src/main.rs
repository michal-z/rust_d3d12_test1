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
    vertex_buffer_srv: D3D12_CPU_DESCRIPTOR_HANDLE,
}

impl App {
    fn new() -> Self {
        let app_name = CString::new("d3d12_simple").unwrap();

        let window = create_window(&app_name, 1920, 1080);
        let mut dx = Dx12Context::new(window);
        let cmdlist = dx.get_and_reset_command_list();

        let pso = dx.create_graphics_pipeline(
            &mut D3D12_GRAPHICS_PIPELINE_STATE_DESC {
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
        );

        let (vertex_buffer, vertex_buffer_srv) = Self::create_vertex_buffer(&mut dx);

        cmdlist.close();
        dx.cmdqueue
            .execute_command_list(&[cmdlist.as_raw() as *mut _]);
        dx.finish();

        Self {
            app_name,
            dx,
            frame_stats: FrameStats::new(),
            pso,
            vertex_buffer,
            vertex_buffer_srv,
        }
    }

    fn destroy(&mut self) {
        self.dx.finish();
        self.dx.destroy();
    }

    fn create_vertex_buffer(
        dx: &mut Dx12Context,
    ) -> (Dx12ResourceHandle, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let vertex_buffer_handle = dx.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            D3D12_HEAP_FLAG_NONE,
            &Dx12ResourceDesc::buffer(1024),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );
        let vertex_buffer = dx.get_resource(vertex_buffer_handle);

        let (cpu_addr, buffer, offset) = dx.allocate_upload_buffer_region(127);
        let cpu_addr = cpu_addr as *mut Vec3;
        unsafe {
            *cpu_addr = Vec3::new(-0.1, -0.7, 0.0);
            *cpu_addr.offset(1) = Vec3::new(0.0, 0.7, 0.0);
            *cpu_addr.offset(2) = Vec3::new(0.7, -0.7, 0.0);
        }
        let cmdlist = dx.get_current_command_list();
        cmdlist.copy_buffer_region(vertex_buffer, 0, buffer, offset, 48);
        dx.transition_barrier(
            cmdlist,
            vertex_buffer_handle,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
        );

        let vertex_buffer_srv =
            dx.allocate_cpu_descriptors(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1);
        dx.device.create_shader_resource_view(
            Some(vertex_buffer),
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: DX12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                u: unsafe {
                    let mut u: D3D12_SHADER_RESOURCE_VIEW_DESC_u = mem::zeroed();
                    u.Buffer_mut().NumElements = 3;
                    u.Buffer_mut().StructureByteStride = 16;
                    u
                },
            }),
            vertex_buffer_srv,
        );

        (vertex_buffer_handle, vertex_buffer_srv)
    }

    fn draw(&mut self) {
        let dx = &mut self.dx;
        let (back_buffer, back_buffer_rtv) = dx.get_back_buffer();
        let cmdlist = dx.get_and_reset_command_list();
        cmdlist.rs_set_viewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: dx.resolution[0] as f32,
            Height: dx.resolution[1] as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        cmdlist.rs_set_scissor_rects(&[D3D12_RECT {
            top: 0,
            left: 0,
            right: dx.resolution[0] as i32,
            bottom: dx.resolution[1] as i32,
        }]);
        dx.transition_barrier(cmdlist, back_buffer, D3D12_RESOURCE_STATE_RENDER_TARGET);
        cmdlist.om_set_render_target(back_buffer_rtv, None);
        cmdlist.clear_render_target_view(
            back_buffer_rtv,
            &[0.2 as f32, 0.4 as f32, 0.8 as f32, 1.0 as f32],
            &[],
        );
        dx.set_graphics_pipeline(cmdlist, self.pso);
        cmdlist.ia_set_primitive_topology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        cmdlist.set_graphics_root_descriptor_table(
            0,
            dx.copy_descriptors_to_gpu_heap(1, self.vertex_buffer_srv),
        );
        cmdlist.draw_instanced(3, 1, 0, 0);
        dx.transition_barrier(cmdlist, back_buffer, D3D12_RESOURCE_STATE_PRESENT);
        cmdlist.close();

        dx.cmdqueue
            .execute_command_list(&[cmdlist.as_raw() as *mut _]);
        dx.present_frame(0);
    }

    fn run(&mut self) {
        while handle_window_messages() {
            self.frame_stats.update(self.dx.window, &self.app_name);
            self.draw();
        }
        self.destroy();
    }
}

fn main() {
    App::new().run();
}
