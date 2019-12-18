use crate::util::WeakPtr;
use std::mem;
use std::option::Option;
use std::ptr;
use winapi::um::d3d12::*;
use winapi::um::winnt::HRESULT;

pub type Dx12Device = WeakPtr<ID3D12Device2>;
pub type Dx12CommandQueue = WeakPtr<ID3D12CommandQueue>;
pub type Dx12GraphicsCommandList = WeakPtr<ID3D12GraphicsCommandList1>;
pub type Dx12Resource = WeakPtr<ID3D12Resource>;

impl Dx12Device {
    #[inline]
    pub fn create_shader_resource_view(
        &self,
        resource: Option<Dx12Resource>,
        desc: Option<&D3D12_SHADER_RESOURCE_VIEW_DESC>,
        dest_descriptor: D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let resource = match resource {
            Some(r) => r.as_raw(),
            None => ptr::null_mut(),
        };
        let desc = match desc {
            Some(d) => d as *const _,
            None => ptr::null(),
        };
        unsafe { self.CreateShaderResourceView(resource, desc, dest_descriptor) };
    }
}

impl Dx12GraphicsCommandList {
    #[inline]
    pub fn rs_set_viewports(&self, viewports: &[D3D12_VIEWPORT]) {
        unsafe { self.RSSetViewports(viewports.len() as u32, viewports.as_ptr() as *const _) };
    }

    #[inline]
    pub fn rs_set_scissor_rects(&self, rects: &[D3D12_RECT]) {
        unsafe { self.RSSetScissorRects(rects.len() as u32, rects.as_ptr() as *const _) };
    }

    #[inline]
    pub fn om_set_render_target(
        &self,
        render_target_descriptor: D3D12_CPU_DESCRIPTOR_HANDLE,
        depth_stencil_descriptor: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    ) {
        let ds = if depth_stencil_descriptor.is_none() {
            ptr::null()
        } else {
            &depth_stencil_descriptor.unwrap()
        };
        unsafe { self.OMSetRenderTargets(1, &render_target_descriptor, 1, ds) };
    }

    #[inline]
    pub fn clear_render_target_view(
        &self,
        render_target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
        color_rgba: &[f32; 4],
        rects: &[D3D12_RECT],
    ) {
        let (num_rects, rects) = if rects.is_empty() {
            (0 as u32, ptr::null())
        } else {
            (rects.len() as u32, rects.as_ptr() as *const _)
        };
        unsafe {
            self.ClearRenderTargetView(
                render_target_view,
                color_rgba.as_ptr() as *const _,
                num_rects,
                rects,
            )
        };
    }

    #[inline]
    pub fn copy_buffer_region(
        &self,
        dst_buffer: Dx12Resource,
        dst_offset: u64,
        src_buffer: Dx12Resource,
        src_offset: u64,
        num_bytes: u64,
    ) {
        unsafe {
            self.CopyBufferRegion(
                dst_buffer.as_raw(),
                dst_offset,
                src_buffer.as_raw(),
                src_offset,
                num_bytes,
            )
        };
    }

    #[inline]
    pub fn ia_set_vertex_buffers(&self, start_slot: u32, views: &[D3D12_VERTEX_BUFFER_VIEW]) {
        assert!(!views.is_empty());
        unsafe {
            self.IASetVertexBuffers(start_slot, views.len() as u32, views.as_ptr() as *const _)
        };
    }

    #[inline]
    pub fn ia_set_primitive_topology(&self, primitive_topology: D3D12_PRIMITIVE_TOPOLOGY) {
        unsafe { self.IASetPrimitiveTopology(primitive_topology) };
    }

    #[inline]
    pub fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    ) {
        unsafe {
            self.DrawInstanced(
                vertex_count_per_instance,
                instance_count,
                start_vertex_location,
                start_instance_location,
            )
        };
    }

    #[inline]
    pub fn set_graphics_root_shader_resource_view(
        &self,
        root_parameter_index: u32,
        buffer_location: D3D12_GPU_VIRTUAL_ADDRESS,
    ) {
        unsafe { self.SetGraphicsRootShaderResourceView(root_parameter_index, buffer_location) };
    }

    #[inline]
    pub fn set_graphics_root_descriptor_table(
        &self,
        root_parameter_index: u32,
        base_descriptor: D3D12_GPU_DESCRIPTOR_HANDLE,
    ) {
        unsafe { self.SetGraphicsRootDescriptorTable(root_parameter_index, base_descriptor) };
    }

    #[inline]
    pub fn set_graphics_root_32bit_constant(
        &self,
        root_parameter_index: u32,
        src_data: u32,
        dest_offset_in_32bit_values: u32,
    ) {
        unsafe {
            self.SetGraphicsRoot32BitConstant(
                root_parameter_index,
                src_data,
                dest_offset_in_32bit_values,
            )
        };
    }

    #[inline]
    pub fn set_graphics_root_32bit_constants<T>(
        &self,
        root_parameter_index: u32,
        src_data: &[T],
        dest_offset_in_32bit_values: u32,
    ) {
        assert_eq!(mem::size_of::<T>(), 4);
        assert!(!src_data.is_empty());
        unsafe {
            self.SetGraphicsRoot32BitConstants(
                root_parameter_index,
                src_data.len() as u32,
                src_data.as_ptr() as *const _,
                dest_offset_in_32bit_values,
            )
        };
    }

    #[inline]
    pub fn close(&self) -> HRESULT {
        let hr = unsafe { self.Close() };
        assert_eq!(hr, 0);
        hr
    }
}

impl Dx12Resource {
    #[inline]
    pub fn get_gpu_virtual_address(&self) -> D3D12_GPU_VIRTUAL_ADDRESS {
        unsafe { self.GetGPUVirtualAddress() }
    }
}

impl Dx12CommandQueue {
    #[inline]
    pub fn execute_command_list(&self, command_lists: &[*mut ID3D12CommandList]) {
        assert!(!command_lists.is_empty());
        unsafe { self.ExecuteCommandLists(command_lists.len() as u32, command_lists.as_ptr()) };
    }
}
