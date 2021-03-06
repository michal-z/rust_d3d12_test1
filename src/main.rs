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
mod d3d12;

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

struct App {
    app_name: CString,
    frame_stats: util::FrameStats,
    d3d: d3d12::Context,
    pso: d3d12::PipelineHandle,
    vertex_buffer: d3d12::ResourceHandle,
    index_buffer: d3d12::ResourceHandle,
    transform_buffer: d3d12::ResourceHandle,
    vertex_buffer_srv: D3D12_CPU_DESCRIPTOR_HANDLE,
    index_buffer_srv: D3D12_CPU_DESCRIPTOR_HANDLE,
    transform_buffer_srv: D3D12_CPU_DESCRIPTOR_HANDLE,
}

impl App {
    fn new() -> Self {
        let app_name = CString::new("d3d12_simple").unwrap();
        let window = util::create_window(&app_name, 1920, 1080);
        let mut d3d = d3d12::Context::new(window);
        let cmdlist = d3d.cmdlist;

        d3d.begin_frame();

        let pso = d3d.create_graphics_pipeline(
            &mut D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                RasterizerState: d3d12::RasterizerDesc::default(),
                BlendState: d3d12::BlendDesc::default(),
                RTVFormats: [DXGI_FORMAT_R8G8B8A8_UNORM, 0, 0, 0, 0, 0, 0, 0],
                DepthStencilState: {
                    let mut desc = d3d12::DepthStencilDesc::default();
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

        let (vertex_buffer, vertex_buffer_srv) = Self::create_vertex_buffer(&mut d3d);
        let (index_buffer, index_buffer_srv) = Self::create_index_buffer(&mut d3d);
        let (transform_buffer, transform_buffer_srv) = Self::create_transform_buffer(&mut d3d);

        d3d.end_frame(0);
        d3d.wait_for_gpu();

        Self {
            app_name,
            d3d,
            frame_stats: util::FrameStats::new(),
            pso,
            vertex_buffer,
            vertex_buffer_srv,
            index_buffer,
            index_buffer_srv,
            transform_buffer,
            transform_buffer_srv,
        }
    }

    fn destroy(&mut self) {
        self.d3d.wait_for_gpu();
        self.d3d.destroy();
    }

    fn create_vertex_buffer(
        d3d: &mut d3d12::Context,
    ) -> (d3d12::ResourceHandle, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let data = vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                color: [0.0, 0.0, 0.0],
            },
            Vertex {
                position: [-0.1, -0.7, 0.0],
                color: [1.0, 0.0, 0.0],
            },
            Vertex {
                position: [0.0, 0.7, 0.0],
                color: [0.0, 1.0, 0.0],
            },
            Vertex {
                position: [0.7, -0.7, 0.0],
                color: [0.0, 0.0, 1.0],
            },
            Vertex {
                position: [0.0, 0.0, 0.0],
                color: [0.0, 0.0, 0.0],
            },
            Vertex {
                position: [-1.0, -1.0, 0.0],
                color: [1.0, 1.0, 0.0],
            },
            Vertex {
                position: [-0.7, -0.7, 0.0],
                color: [0.0, 1.0, 1.0],
            },
            Vertex {
                position: [-0.7, -1.0, 0.0],
                color: [1.0, 0.0, 1.0],
            },
        ];

        let buffer_handle = Self::create_buffer(
            d3d,
            data.as_ptr() as *const u8,
            data.len() * mem::size_of::<Vertex>(),
        );
        let buffer = d3d.resource(buffer_handle);
        let buffer_srv = d3d.allocate_cpu_descriptors(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1);

        d3d.device.create_shader_resource_view(
            Some(buffer),
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: d3d12::DEFAULT_SHADER_4_COMPONENT_MAPPING,
                u: unsafe {
                    let mut u: D3D12_SHADER_RESOURCE_VIEW_DESC_u = mem::zeroed();
                    u.Buffer_mut().NumElements = data.len() as u32;
                    u.Buffer_mut().StructureByteStride = mem::size_of::<Vertex>() as u32;
                    u
                },
            }),
            buffer_srv,
        );

        (buffer_handle, buffer_srv)
    }

    fn create_index_buffer(
        d3d: &mut d3d12::Context,
    ) -> (d3d12::ResourceHandle, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let data = vec![0 as u32, 0, 0, 0, 1, 2, 0, 0, 1, 2, 0];

        let buffer_handle = Self::create_buffer(
            d3d,
            data.as_ptr() as *const u8,
            data.len() * mem::size_of::<u32>(),
        );
        let buffer = d3d.resource(buffer_handle);
        let buffer_srv = d3d.allocate_cpu_descriptors(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1);

        d3d.device.create_shader_resource_view(
            Some(buffer),
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R32_UINT,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: d3d12::DEFAULT_SHADER_4_COMPONENT_MAPPING,
                u: unsafe {
                    let mut u: D3D12_SHADER_RESOURCE_VIEW_DESC_u = mem::zeroed();
                    u.Buffer_mut().NumElements = data.len() as u32;
                    u
                },
            }),
            buffer_srv,
        );

        (buffer_handle, buffer_srv)
    }

    fn create_transform_buffer(
        d3d: &mut d3d12::Context,
    ) -> (d3d12::ResourceHandle, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let data = vec![
            Mat4::from_translation(Vec3::new(0.2, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.4, 0.0, 0.0)),
        ];

        let buffer_handle = Self::create_buffer(
            d3d,
            data.as_ptr() as *const u8,
            data.len() * mem::size_of::<Mat4>(),
        );
        let buffer = d3d.resource(buffer_handle);
        let buffer_srv = d3d.allocate_cpu_descriptors(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1);

        d3d.device.create_shader_resource_view(
            Some(buffer),
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: d3d12::DEFAULT_SHADER_4_COMPONENT_MAPPING,
                u: unsafe {
                    let mut u: D3D12_SHADER_RESOURCE_VIEW_DESC_u = mem::zeroed();
                    u.Buffer_mut().NumElements = data.len() as u32;
                    u.Buffer_mut().StructureByteStride = mem::size_of::<Mat4>() as u32;
                    u
                },
            }),
            buffer_srv,
        );

        (buffer_handle, buffer_srv)
    }

    fn create_buffer(
        d3d: &mut d3d12::Context,
        data: *const u8,
        data_size: usize,
    ) -> d3d12::ResourceHandle {
        let buffer_handle = d3d.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            D3D12_HEAP_FLAG_NONE,
            &d3d12::ResourceDesc::buffer(data_size as u64),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );
        let buffer = d3d.resource(buffer_handle);

        let (cpu_addr, upload_buffer, upload_offset) =
            d3d.allocate_upload_buffer_region(data_size as u32);
        let cpu_addr = cpu_addr as *mut u8;

        unsafe { ptr::copy(data, cpu_addr, data_size) };

        d3d.cmdlist
            .copy_buffer_region(buffer, 0, upload_buffer, upload_offset, data_size as u64);
        d3d.cmd_transition_barrier(
            buffer_handle,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
        );

        buffer_handle
    }

    fn draw(&mut self) {
        let d3d = &mut self.d3d;
        let (back_buffer, back_buffer_rtv) = d3d.back_buffer();
        let cmdlist = d3d.begin_frame();

        cmdlist.rs_set_viewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: d3d.resolution[0] as f32,
            Height: d3d.resolution[1] as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        cmdlist.rs_set_scissor_rects(&[D3D12_RECT {
            top: 0,
            left: 0,
            right: d3d.resolution[0] as i32,
            bottom: d3d.resolution[1] as i32,
        }]);
        d3d.cmd_transition_barrier(back_buffer, D3D12_RESOURCE_STATE_RENDER_TARGET);
        cmdlist.om_set_render_target(back_buffer_rtv, None);
        cmdlist.clear_render_target_view(back_buffer_rtv, &[0.2 as f32, 0.4, 0.8, 1.0], &[]);
        cmdlist.ia_set_primitive_topology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

        d3d.cmd_set_graphics_pipeline(self.pso);
        cmdlist.set_graphics_root_descriptor_table(1, {
            let table_base = d3d.copy_descriptors_to_gpu_heap(1, self.vertex_buffer_srv);
            d3d.copy_descriptors_to_gpu_heap(1, self.index_buffer_srv);
            d3d.copy_descriptors_to_gpu_heap(1, self.transform_buffer_srv);
            table_base
        });

        cmdlist.set_graphics_root_32bit_constants(0, &[3, 1, 0], 0);
        cmdlist.draw_instanced(3, 1, 0, 0);

        cmdlist.set_graphics_root_32bit_constants(0, &[8, 5, 1], 0);
        cmdlist.draw_instanced(3, 1, 0, 0);

        d3d.cmd_transition_barrier(back_buffer, D3D12_RESOURCE_STATE_PRESENT);

        d3d.end_frame(0);
    }

    fn run(&mut self) {
        while util::handle_window_messages() {
            self.frame_stats.update(self.d3d.window, &self.app_name);
            self.draw();
        }
        self.destroy();
    }
}

fn main() {
    App::new().run();
}
