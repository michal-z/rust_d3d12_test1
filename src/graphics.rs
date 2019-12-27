use crate::d3d12::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::fs;
use std::hash::Hasher;
use std::mem;
use std::ops::Deref;
use std::ptr;
use std::slice;
use winapi::ctypes::c_void;
use winapi::shared::dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_FLIP_DISCARD};
use winapi::shared::dxgi1_3::{CreateDXGIFactory2, DXGI_CREATE_FACTORY_DEBUG};
use winapi::shared::dxgi1_4::{IDXGIFactory4, IDXGISwapChain3};
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::{DXGI_SAMPLE_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::d3d12::*;
#[cfg(debug_assertions)]
use winapi::um::d3d12sdklayers::{ID3D12Debug, ID3D12Debug1};
use winapi::um::d3dcommon::D3D_FEATURE_LEVEL_11_1;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::{CreateEventExA, WaitForSingleObject};
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::{EVENT_ALL_ACCESS, HANDLE};
use winapi::um::winuser::GetClientRect;
use winapi::Interface;

const MAX_NUM_RESOURCES: usize = 256;
const MAX_NUM_PIPELINES: usize = 256;
const INVALID_PIPELINE: Dx12PipelineHandle = Dx12PipelineHandle {
    index: 0,
    generation: 0,
};

pub const DX12_DEFAULT_SHADER_4_COMPONENT_MAPPING: u32 =
    0 | (1 << 3) | (2 << (3 * 2)) | (3 << (3 * 3)) | (1 << (3 * 4));

pub struct Dx12Context {
    pub device: Dx12Device,
    pub cmdqueue: Dx12CommandQueue,
    pub frame_index: u32,
    pub resolution: [u32; 2],
    pub window: HWND,
    cmdlist: Dx12GraphicsCommandList,
    cmdallocs: [WeakPtr<ID3D12CommandAllocator>; 2],
    swapchain: WeakPtr<IDXGISwapChain3>,
    rtv_heap: DescriptorHeap,
    dsv_heap: DescriptorHeap,
    cpu_cbv_srv_uav_heap: DescriptorHeap,
    gpu_cbv_srv_uav_heaps: [DescriptorHeap; 2],
    gpu_upload_memory_heaps: [GpuMemoryHeap; 2],
    swap_buffers: [Dx12ResourceHandle; 4],
    frame_fence: WeakPtr<ID3D12Fence>,
    frame_fence_event: HANDLE,
    num_frames: u64,
    back_buffer_index: u32,
    resource_pool: ResourcePool,
    pipeline_pool: PipelinePool,
    current_pipeline: Dx12PipelineHandle,
}

#[derive(Copy, Clone, PartialEq)]
pub struct Dx12ResourceHandle {
    index: u16,
    generation: u16,
}

#[derive(Copy, Clone, PartialEq)]
pub struct Dx12PipelineHandle {
    index: u16,
    generation: u16,
}

#[derive(Copy, Clone)]
struct ResourceState {
    ptr: WeakPtr<ID3D12Resource>,
    state: D3D12_RESOURCE_STATES,
    format: DXGI_FORMAT,
}

#[derive(Copy, Clone)]
struct PipelineState {
    pso: WeakPtr<ID3D12PipelineState>,
    rsignature: WeakPtr<ID3D12RootSignature>,
}

struct ResourcePool {
    resources: Vec<ResourceState>,
    generations: Vec<u16>,
}

struct PipelinePool {
    pipelines: Vec<PipelineState>,
    generations: Vec<u16>,
    map: HashMap<u64, Dx12PipelineHandle>,
}

struct DescriptorHeap {
    heap: WeakPtr<ID3D12DescriptorHeap>,
    cpu_base: D3D12_CPU_DESCRIPTOR_HANDLE,
    gpu_base: D3D12_GPU_DESCRIPTOR_HANDLE,
    size: u32,
    capacity: u32,
    descriptor_size: u32,
}

struct GpuMemoryHeap {
    heap: WeakPtr<ID3D12Resource>,
    cpu_base: *mut u8,
    gpu_base: D3D12_GPU_VIRTUAL_ADDRESS,
    size: u32,
    capacity: u32,
}

pub struct Dx12ResourceBarrier;
pub struct Dx12RasterizerDesc;
pub struct Dx12BlendDesc;
pub struct Dx12DepthStencilDesc;
pub struct Dx12ResourceDesc;
pub struct Dx12HeapProperties;
pub struct Dx12InputElementDesc;

impl Dx12ResourceBarrier {
    pub fn transition(
        resource: Dx12Resource,
        state_before: D3D12_RESOURCE_STATES,
        state_after: D3D12_RESOURCE_STATES,
    ) -> D3D12_RESOURCE_BARRIER {
        let mut barrier: D3D12_RESOURCE_BARRIER = unsafe { mem::zeroed() };
        barrier.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        barrier.Flags = D3D12_RESOURCE_FLAG_NONE;
        let mut transition = unsafe { barrier.u.Transition_mut() };
        transition.pResource = resource.as_raw();
        transition.StateBefore = state_before;
        transition.StateAfter = state_after;
        transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        barrier
    }
}

impl Dx12RasterizerDesc {
    pub fn default() -> D3D12_RASTERIZER_DESC {
        D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_BACK,
            FrontCounterClockwise: 0,
            DepthBias: D3D12_DEFAULT_DEPTH_BIAS as i32,
            DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
            SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
            DepthClipEnable: 1,
            MultisampleEnable: 0,
            AntialiasedLineEnable: 0,
            ForcedSampleCount: 0,
            ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        }
    }
}

impl Dx12BlendDesc {
    pub fn default() -> D3D12_BLEND_DESC {
        let rt_blend_desc = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: 0,
            LogicOpEnable: 0,
            SrcBlend: D3D12_BLEND_ONE,
            DestBlend: D3D12_BLEND_ZERO,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_ZERO,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: 0x0f,
        };
        D3D12_BLEND_DESC {
            AlphaToCoverageEnable: 0,
            IndependentBlendEnable: 0,
            RenderTarget: [
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
                rt_blend_desc,
            ],
        }
    }
}

impl Dx12ResourceDesc {
    pub fn buffer(size: u64) -> D3D12_RESOURCE_DESC {
        D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        }
    }
}

impl Dx12DepthStencilDesc {
    pub fn default() -> D3D12_DEPTH_STENCIL_DESC {
        let ds_op_desc = D3D12_DEPTH_STENCILOP_DESC {
            StencilFailOp: D3D12_STENCIL_OP_KEEP,
            StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
            StencilPassOp: D3D12_STENCIL_OP_KEEP,
            StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
        };
        D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: 1,
            DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D12_COMPARISON_FUNC_LESS,
            StencilEnable: 0,
            StencilReadMask: D3D12_DEFAULT_STENCIL_READ_MASK as u8,
            StencilWriteMask: D3D12_DEFAULT_STENCIL_WRITE_MASK as u8,
            FrontFace: ds_op_desc,
            BackFace: ds_op_desc,
        }
    }
}

impl Dx12HeapProperties {
    pub fn new(heap_type: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
        D3D12_HEAP_PROPERTIES {
            Type: heap_type,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        }
    }
}

impl Dx12InputElementDesc {
    pub fn new(name: &CString, format: DXGI_FORMAT, offset: u32) -> D3D12_INPUT_ELEMENT_DESC {
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: name.as_ptr(),
            SemanticIndex: 0,
            Format: format,
            InputSlot: 0,
            AlignedByteOffset: offset,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }
    }
}

impl ResourceState {
    #[inline]
    fn new() -> Self {
        Self {
            ptr: WeakPtr::new(),
            state: D3D12_RESOURCE_STATE_COMMON,
            format: DXGI_FORMAT_UNKNOWN,
        }
    }
}

impl PipelineState {
    #[inline]
    fn new() -> Self {
        Self {
            pso: WeakPtr::new(),
            rsignature: WeakPtr::new(),
        }
    }
}

impl ResourcePool {
    fn new() -> Self {
        Self {
            resources: vec![ResourceState::new(); MAX_NUM_RESOURCES + 1],
            generations: vec![0; MAX_NUM_RESOURCES + 1],
        }
    }

    fn destroy(&mut self) {
        for i in 0..self.resources.len() {
            self.resources[i].ptr.release();
            self.generations[i] = 0;
        }
    }

    fn add(
        &mut self,
        resource: WeakPtr<ID3D12Resource>,
        initial_state: D3D12_RESOURCE_STATES,
        format: DXGI_FORMAT,
    ) -> Dx12ResourceHandle {
        let mut slot_idx = 0;
        for i in 1..self.resources.len() {
            if self.resources[i].ptr.is_null() {
                slot_idx = i;
                break;
            }
        }
        assert!(slot_idx > 0 && slot_idx <= MAX_NUM_RESOURCES);

        self.resources[slot_idx].ptr = resource;
        self.resources[slot_idx].state = initial_state;
        self.resources[slot_idx].format = format;

        Dx12ResourceHandle {
            index: slot_idx as u16,
            generation: {
                self.generations[slot_idx] += 1;
                self.generations[slot_idx]
            },
        }
    }
}

impl PipelinePool {
    fn new() -> Self {
        Self {
            pipelines: vec![PipelineState::new(); MAX_NUM_PIPELINES + 1],
            generations: vec![0; MAX_NUM_PIPELINES + 1],
            map: HashMap::new(),
        }
    }

    fn destroy(&mut self) {
        for i in 0..self.pipelines.len() {
            self.pipelines[i].pso.release();
            self.pipelines[i].rsignature.release();
            self.generations[i] = 0;
        }
        self.map.clear();
    }

    fn add(
        &mut self,
        pso: WeakPtr<ID3D12PipelineState>,
        rsignature: WeakPtr<ID3D12RootSignature>,
    ) -> Dx12PipelineHandle {
        let mut slot_idx = 0;
        for i in 1..self.pipelines.len() {
            if self.pipelines[i].pso.is_null() {
                slot_idx = i;
                break;
            }
        }
        assert!(slot_idx > 0 && slot_idx <= MAX_NUM_PIPELINES);

        self.pipelines[slot_idx].pso = pso;
        self.pipelines[slot_idx].rsignature = rsignature;

        Dx12PipelineHandle {
            index: slot_idx as u16,
            generation: {
                self.generations[slot_idx] += 1;
                self.generations[slot_idx]
            },
        }
    }
}

impl Dx12Context {
    pub fn new(window: HWND) -> Self {
        // Create DXGI factory.
        let mut factory = {
            let mut rfactory: *mut IDXGIFactory4 = ptr::null_mut();
            vhr!(CreateDXGIFactory2(
                DXGI_CREATE_FACTORY_DEBUG,
                &IDXGIFactory4::uuidof(),
                &mut rfactory as *mut *mut _ as *mut *mut c_void,
            ));
            WeakPtr::from_raw(rfactory)
        };

        // Debug layer.
        #[cfg(debug_assertions)]
        unsafe {
            let mut rdbg: *mut ID3D12Debug = ptr::null_mut();
            D3D12GetDebugInterface(
                &ID3D12Debug::uuidof(),
                &mut rdbg as *mut *mut _ as *mut *mut c_void,
            );
            if !rdbg.is_null() {
                let mut dbg = WeakPtr::from_raw(rdbg);
                dbg.EnableDebugLayer();

                let mut rdbg1: *mut ID3D12Debug1 = ptr::null_mut();
                dbg.QueryInterface(
                    &ID3D12Debug1::uuidof(),
                    &mut rdbg1 as *mut *mut _ as *mut *mut c_void,
                );
                dbg.release();
                if !rdbg1.is_null() {
                    let mut dbg1 = WeakPtr::from_raw(rdbg1);
                    dbg1.SetEnableGPUBasedValidation(1);
                    dbg1.release();
                }
            }
        }

        // Create Direct3D12 device.
        let device = {
            let mut rdevice: *mut ID3D12Device2 = ptr::null_mut();
            vhr!(D3D12CreateDevice(
                ptr::null_mut(),
                D3D_FEATURE_LEVEL_11_1,
                &ID3D12Device2::uuidof(),
                &mut rdevice as *mut *mut _ as *mut *mut c_void,
            ));
            WeakPtr::from_raw(rdevice)
        };

        // Create command queue.
        let cmdqueue = {
            let mut rcmdqueue: *mut ID3D12CommandQueue = ptr::null_mut();
            vhr!(device.CreateCommandQueue(
                &D3D12_COMMAND_QUEUE_DESC {
                    Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
                    Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL as i32,
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    NodeMask: 0,
                },
                &ID3D12CommandQueue::uuidof(),
                &mut rcmdqueue as *mut *mut _ as *mut *mut c_void,
            ));
            WeakPtr::from_raw(rcmdqueue)
        };

        // Create swap chain.
        let swapchain = {
            let mut swapchain1 = {
                let mut desc: DXGI_SWAP_CHAIN_DESC = unsafe { mem::zeroed() };
                desc.BufferCount = 4;
                desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
                desc.OutputWindow = window;
                desc.SampleDesc.Count = 1;
                desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
                desc.Windowed = 1;

                let mut rswapchain1: *mut IDXGISwapChain = ptr::null_mut();
                vhr!(factory.CreateSwapChain(
                    cmdqueue.as_raw() as *mut _ as *mut IUnknown,
                    &mut desc,
                    &mut rswapchain1,
                ));
                factory.release();
                WeakPtr::from_raw(rswapchain1)
            };

            let mut rswapchain3: *mut IDXGISwapChain3 = ptr::null_mut();
            vhr!(swapchain1.QueryInterface(
                &IDXGISwapChain3::uuidof(),
                &mut rswapchain3 as *mut *mut _ as *mut *mut c_void,
            ));
            swapchain1.release();
            WeakPtr::from_raw(rswapchain3)
        };

        // Create command allocators.
        let cmdallocs = {
            let mut rcmdallocs: [*mut ID3D12CommandAllocator; 2] =
                [ptr::null_mut(), ptr::null_mut()];

            for i in 0..rcmdallocs.len() {
                vhr!(device.CreateCommandAllocator(
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &ID3D12CommandAllocator::uuidof(),
                    &mut rcmdallocs[i] as *mut *mut _ as *mut *mut c_void,
                ));
            }
            [
                WeakPtr::from_raw(rcmdallocs[0]),
                WeakPtr::from_raw(rcmdallocs[1]),
            ]
        };

        // Create descriptor heaps.
        let mut rtv_heap = DescriptorHeap::new(
            device,
            1024,
            D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let dsv_heap = DescriptorHeap::new(
            device,
            1024,
            D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let cpu_cbv_srv_uav_heap = DescriptorHeap::new(
            device,
            16 * 1024,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let gpu_cbv_srv_uav_heaps = [
            DescriptorHeap::new(
                device,
                16 * 1024,
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            ),
            DescriptorHeap::new(
                device,
                16 * 1024,
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            ),
        ];

        // Create upload memory heaps.
        let gpu_upload_memory_heaps = [
            GpuMemoryHeap::new(device, 32 * 1024, D3D12_HEAP_TYPE_UPLOAD),
            GpuMemoryHeap::new(device, 32 * 1024, D3D12_HEAP_TYPE_UPLOAD),
        ];

        let mut resource_pool = ResourcePool::new();
        let pipeline_pool = PipelinePool::new();

        let swap_buffers = {
            let mut rbuffers: [*mut ID3D12Resource; 4] = [ptr::null_mut(); 4];
            let mut handle = rtv_heap.allocate_cpu_descriptors(rbuffers.len() as u32);

            for i in 0..rbuffers.len() {
                vhr!(swapchain.GetBuffer(
                    i as u32,
                    &ID3D12Resource::uuidof(),
                    &mut rbuffers[i] as *mut *mut _ as *mut *mut c_void,
                ));
                unsafe { device.CreateRenderTargetView(rbuffers[i], ptr::null(), handle) };
                handle.ptr += rtv_heap.descriptor_size as usize;
            }
            [
                resource_pool.add(
                    WeakPtr::from_raw(rbuffers[0]),
                    D3D12_RESOURCE_STATE_PRESENT,
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                ),
                resource_pool.add(
                    WeakPtr::from_raw(rbuffers[1]),
                    D3D12_RESOURCE_STATE_PRESENT,
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                ),
                resource_pool.add(
                    WeakPtr::from_raw(rbuffers[2]),
                    D3D12_RESOURCE_STATE_PRESENT,
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                ),
                resource_pool.add(
                    WeakPtr::from_raw(rbuffers[3]),
                    D3D12_RESOURCE_STATE_PRESENT,
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                ),
            ]
        };

        let cmdlist = {
            let mut rcmdlist: *mut ID3D12GraphicsCommandList1 = ptr::null_mut();
            vhr!(device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                cmdallocs[0].as_raw(),
                ptr::null_mut(),
                &ID3D12GraphicsCommandList1::uuidof(),
                &mut rcmdlist as *mut *mut _ as *mut *mut c_void,
            ));
            Dx12GraphicsCommandList::from_raw(rcmdlist)
        };
        vhr!(cmdlist.Close());

        let frame_fence = {
            let mut rfence: *mut ID3D12Fence = ptr::null_mut();
            vhr!(device.CreateFence(
                0,
                D3D12_FENCE_FLAG_NONE,
                &ID3D12Fence::uuidof(),
                &mut rfence as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(rfence)
        };

        let frame_fence_event =
            unsafe { CreateEventExA(ptr::null_mut(), ptr::null(), 0, EVENT_ALL_ACCESS) };

        let back_buffer_index = unsafe { swapchain.GetCurrentBackBufferIndex() };

        Self {
            device,
            cmdqueue,
            swapchain,
            cmdallocs,
            rtv_heap,
            dsv_heap,
            cpu_cbv_srv_uav_heap,
            gpu_cbv_srv_uav_heaps,
            gpu_upload_memory_heaps,
            swap_buffers,
            cmdlist,
            frame_fence,
            frame_fence_event,
            num_frames: 0,
            frame_index: 0,
            back_buffer_index,
            resolution: unsafe {
                let mut rect: RECT = mem::zeroed();
                GetClientRect(window, &mut rect as *mut RECT);
                [rect.right as u32, rect.bottom as u32]
            },
            window,
            resource_pool,
            pipeline_pool,
            current_pipeline: INVALID_PIPELINE,
        }
    }

    pub fn destroy(&mut self) {
        self.resource_pool.destroy();
        self.pipeline_pool.destroy();
        self.device.release();
        self.cmdqueue.release();
        self.swapchain.release();
        self.cmdallocs[0].release();
        self.cmdallocs[1].release();
        self.rtv_heap.heap.release();
        self.dsv_heap.heap.release();
        self.cpu_cbv_srv_uav_heap.heap.release();
        self.gpu_cbv_srv_uav_heaps[0].heap.release();
        self.gpu_cbv_srv_uav_heaps[1].heap.release();
        self.gpu_upload_memory_heaps[0].heap.release();
        self.gpu_upload_memory_heaps[1].heap.release();
        self.cmdlist.release();
        self.frame_fence.release();
        unsafe { CloseHandle(self.frame_fence_event) };
        self.frame_fence_event = ptr::null_mut();
    }

    #[inline]
    pub fn current_command_list(&self) -> Dx12GraphicsCommandList {
        self.cmdlist
    }

    #[inline]
    fn validate_resource_state(&self, handle: Dx12ResourceHandle) {
        let index = handle.index as usize;
        assert!(index > 0 && index <= MAX_NUM_RESOURCES);
        assert!(handle.generation == self.resource_pool.generations[index]);
        assert!(!self.resource_pool.resources[index].ptr.is_null());
    }

    #[inline]
    fn validate_pipeline_state(&self, handle: Dx12PipelineHandle) {
        let index = handle.index as usize;
        assert!(index > 0 && index <= MAX_NUM_PIPELINES);
        assert!(handle.generation == self.pipeline_pool.generations[index]);
        assert!(!self.pipeline_pool.pipelines[index].pso.is_null());
        assert!(!self.pipeline_pool.pipelines[index].rsignature.is_null());
    }

    #[inline]
    pub fn resource(&self, handle: Dx12ResourceHandle) -> WeakPtr<ID3D12Resource> {
        self.validate_resource_state(handle);
        self.resource_pool.resources[handle.index as usize].ptr
    }

    #[inline]
    fn pipeline_state(&self, handle: Dx12PipelineHandle) -> &PipelineState {
        self.validate_pipeline_state(handle);
        &self.pipeline_pool.pipelines[handle.index as usize]
    }

    #[inline]
    fn resource_state_mut(&mut self, handle: Dx12ResourceHandle) -> &mut ResourceState {
        self.validate_resource_state(handle);
        &mut self.resource_pool.resources[handle.index as usize]
    }

    pub fn create_committed_resource(
        &mut self,
        heap_type: D3D12_HEAP_TYPE,
        heap_flags: D3D12_HEAP_FLAGS,
        desc: &D3D12_RESOURCE_DESC,
        initial_state: D3D12_RESOURCE_STATES,
        clear_value: Option<&D3D12_CLEAR_VALUE>,
    ) -> Dx12ResourceHandle {
        let resource = {
            let mut resource_raw: *mut ID3D12Resource = ptr::null_mut();
            vhr!(self.device.CreateCommittedResource(
                &Dx12HeapProperties::new(heap_type),
                heap_flags,
                desc,
                initial_state,
                if clear_value.is_none() {
                    ptr::null()
                } else {
                    clear_value.unwrap()
                },
                &ID3D12Resource::uuidof(),
                &mut resource_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(resource_raw)
        };
        self.resource_pool.add(resource, initial_state, desc.Format)
    }

    pub fn destroy_resource(&mut self, handle: Dx12ResourceHandle) {
        let mut resource = self.resource_state_mut(handle);

        let refcount = resource.ptr.release();
        assert!(refcount == 0);

        resource.state = D3D12_RESOURCE_STATE_COMMON;
        resource.format = DXGI_FORMAT_UNKNOWN;
    }

    pub fn transition_barrier(
        &mut self,
        cmdlist: Dx12GraphicsCommandList,
        resource_handle: Dx12ResourceHandle,
        state_after: D3D12_RESOURCE_STATES,
    ) {
        let mut resource = self.resource_state_mut(resource_handle);
        if resource.state != state_after {
            unsafe {
                cmdlist.ResourceBarrier(
                    1,
                    &Dx12ResourceBarrier::transition(resource.ptr, resource.state, state_after),
                )
            };
            resource.state = state_after;
        }
    }

    pub fn set_graphics_pipeline(
        &mut self,
        cmdlist: Dx12GraphicsCommandList,
        handle: Dx12PipelineHandle,
    ) {
        let pipeline_state = self.pipeline_state(handle);
        if handle != self.current_pipeline {
            unsafe {
                cmdlist.SetPipelineState(pipeline_state.pso.as_raw());
                cmdlist.SetGraphicsRootSignature(pipeline_state.rsignature.as_raw());
                self.current_pipeline = handle;
            }
        }
    }

    pub fn create_graphics_pipeline(
        &mut self,
        pso_desc: &mut D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        vs_name: &str,
        ps_name: &str,
    ) -> Dx12PipelineHandle {
        let vs_bytecode = fs::read(format!("data/shaders/{}", vs_name)).unwrap();
        let ps_bytecode = fs::read(format!("data/shaders/{}", ps_name)).unwrap();

        pso_desc.VS = D3D12_SHADER_BYTECODE {
            pShaderBytecode: vs_bytecode.as_ptr() as *const c_void,
            BytecodeLength: vs_bytecode.len(),
        };
        pso_desc.PS = D3D12_SHADER_BYTECODE {
            pShaderBytecode: ps_bytecode.as_ptr() as *const c_void,
            BytecodeLength: ps_bytecode.len(),
        };

        let hash = calc_graphics_pipeline_hash(pso_desc);

        let found = self.pipeline_pool.map.get(&hash);
        if found != None {
            return *found.unwrap();
        }

        let rsignature = {
            let mut rsignature_raw: *mut ID3D12RootSignature = ptr::null_mut();
            vhr!(self.device.CreateRootSignature(
                0,
                vs_bytecode.as_ptr() as *const c_void,
                vs_bytecode.len(),
                &ID3D12RootSignature::uuidof(),
                &mut rsignature_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(rsignature_raw)
        };

        pso_desc.pRootSignature = rsignature.as_raw();

        let pso = {
            let mut pso_raw: *mut ID3D12PipelineState = ptr::null_mut();
            vhr!(self.device.CreateGraphicsPipelineState(
                pso_desc,
                &ID3D12PipelineState::uuidof(),
                &mut pso_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(pso_raw)
        };

        let handle = self.pipeline_pool.add(pso, rsignature);
        self.pipeline_pool.map.insert(hash, handle);
        handle
    }

    pub fn create_compute_pipeline(
        &mut self,
        pso_desc: &mut D3D12_COMPUTE_PIPELINE_STATE_DESC,
        cs_name: &str,
    ) -> Dx12PipelineHandle {
        let cs_bytecode = fs::read(format!("data/shaders/{}", cs_name)).unwrap();

        pso_desc.CS = D3D12_SHADER_BYTECODE {
            pShaderBytecode: cs_bytecode.as_ptr() as *const c_void,
            BytecodeLength: cs_bytecode.len(),
        };

        let hash = calc_compute_pipeline_hash(pso_desc);

        let found = self.pipeline_pool.map.get(&hash);
        if found != None {
            return *found.unwrap();
        }

        let rsignature = {
            let mut rsignature_raw: *mut ID3D12RootSignature = ptr::null_mut();
            vhr!(self.device.CreateRootSignature(
                0,
                cs_bytecode.as_ptr() as *const c_void,
                cs_bytecode.len(),
                &ID3D12RootSignature::uuidof(),
                &mut rsignature_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(rsignature_raw)
        };

        pso_desc.pRootSignature = rsignature.as_raw();

        let pso = {
            let mut pso_raw: *mut ID3D12PipelineState = ptr::null_mut();
            vhr!(self.device.CreateComputePipelineState(
                pso_desc,
                &ID3D12PipelineState::uuidof(),
                &mut pso_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(pso_raw)
        };

        let handle = self.pipeline_pool.add(pso, rsignature);
        self.pipeline_pool.map.insert(hash, handle);
        handle
    }

    pub fn destroy_pipeline(&mut self, handle: Dx12PipelineHandle) {
        self.validate_pipeline_state(handle);

        let mut key_to_remove: u64 = 0;
        for (key, value) in self.pipeline_pool.map.iter() {
            if *value == handle {
                key_to_remove = *key;
                break;
            }
        }
        assert!(key_to_remove != 0);

        self.pipeline_pool.map.remove(&key_to_remove);

        let pipeline = &mut self.pipeline_pool.pipelines[handle.index as usize];
        let pso_refcount = pipeline.pso.release();
        let rsignature_refcount = pipeline.rsignature.release();
        assert!(pso_refcount == 0 && rsignature_refcount == 0);
    }

    pub fn allocate_cpu_descriptors(
        &mut self,
        heap_type: D3D12_DESCRIPTOR_HEAP_TYPE,
        num: u32,
    ) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        match heap_type {
            D3D12_DESCRIPTOR_HEAP_TYPE_RTV => self.rtv_heap.allocate_cpu_descriptors(num),
            D3D12_DESCRIPTOR_HEAP_TYPE_DSV => self.dsv_heap.allocate_cpu_descriptors(num),
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV => {
                self.cpu_cbv_srv_uav_heap.allocate_cpu_descriptors(num)
            }
            _ => {
                assert!(false);
                D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 }
            }
        }
    }

    pub fn allocate_gpu_descriptors(
        &mut self,
        num: u32,
    ) -> (D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_GPU_DESCRIPTOR_HANDLE) {
        self.gpu_cbv_srv_uav_heaps[self.frame_index as usize].allocate_gpu_descriptors(num)
    }

    pub fn allocate_upload_memory(
        &mut self,
        size: u32,
    ) -> (*mut c_void, D3D12_GPU_VIRTUAL_ADDRESS) {
        let index = self.frame_index as usize;

        let (cpu_base, gpu_base) = self.gpu_upload_memory_heaps[index].allocate(size);
        if cpu_base == ptr::null_mut() && gpu_base == 0 {
            self.cmdlist.close();
            self.cmdqueue
                .execute_command_lists(&[self.cmdlist.as_raw() as *mut _]);
            self.finish();
            self.new_command_list();
        }

        let (cpu_base, gpu_base) = self.gpu_upload_memory_heaps[index].allocate(size);
        assert!(cpu_base != ptr::null_mut() && gpu_base != 0);
        (cpu_base, gpu_base)
    }

    pub fn allocate_upload_buffer_region(
        &mut self,
        mut size: u32,
    ) -> (*mut c_void, WeakPtr<ID3D12Resource>, u64) {
        if (size & 0xff) != 0 {
            size = (size + 255) & !0xff;
        }

        let (cpu_addr, _) = self.allocate_upload_memory(size);
        let buffer = self.gpu_upload_memory_heaps[self.frame_index as usize].heap;
        let offset = self.gpu_upload_memory_heaps[self.frame_index as usize].size - size;

        (cpu_addr, buffer, offset as u64)
    }

    #[inline]
    pub fn copy_descriptors_to_gpu_heap(
        &mut self,
        num_descriptors: u32,
        src_cpu_base: D3D12_CPU_DESCRIPTOR_HANDLE,
    ) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let (dest_cpu_base, dest_gpu_base) = self.allocate_gpu_descriptors(num_descriptors);
        unsafe {
            self.device.CopyDescriptorsSimple(
                num_descriptors,
                dest_cpu_base,
                src_cpu_base,
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            )
        };
        dest_gpu_base
    }

    pub fn present_frame(&mut self, swap_interval: u32) {
        self.num_frames += 1;

        vhr!(self.swapchain.Present(swap_interval, 0));
        vhr!(self
            .cmdqueue
            .Signal(self.frame_fence.as_raw(), self.num_frames));

        let gpu_num_frames = unsafe { self.frame_fence.GetCompletedValue() };

        if (self.num_frames - gpu_num_frames) >= 2 {
            let gpu_num_frames = gpu_num_frames + 1;
            vhr!(self
                .frame_fence
                .SetEventOnCompletion(gpu_num_frames, self.frame_fence_event));
            unsafe {
                WaitForSingleObject(self.frame_fence_event, INFINITE);
            }
        }

        self.frame_index = (self.frame_index + 1) % 2;
        self.back_buffer_index = unsafe { self.swapchain.GetCurrentBackBufferIndex() };
        self.gpu_cbv_srv_uav_heaps[self.frame_index as usize].size = 0;
        self.gpu_upload_memory_heaps[self.frame_index as usize].size = 0;
    }

    pub fn new_command_list(&mut self) -> Dx12GraphicsCommandList {
        let index = self.frame_index as usize;
        unsafe {
            self.cmdallocs[index].Reset();
            self.cmdlist
                .Reset(self.cmdallocs[index].as_raw(), ptr::null_mut());
            self.cmdlist.SetDescriptorHeaps(
                1,
                &mut self.gpu_cbv_srv_uav_heaps[index].heap.as_raw()
                    as *mut *mut ID3D12DescriptorHeap,
            );
        }
        self.current_pipeline = INVALID_PIPELINE;
        self.cmdlist
    }

    pub fn finish(&mut self) {
        self.num_frames += 1;

        vhr!(self
            .cmdqueue
            .Signal(self.frame_fence.as_raw(), self.num_frames));
        vhr!(self
            .frame_fence
            .SetEventOnCompletion(self.num_frames, self.frame_fence_event));
        unsafe {
            WaitForSingleObject(self.frame_fence_event, INFINITE);
        }

        self.gpu_cbv_srv_uav_heaps[self.frame_index as usize].size = 0;
        self.gpu_upload_memory_heaps[self.frame_index as usize].size = 0;
    }

    pub fn back_buffer(&self) -> (Dx12ResourceHandle, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let offset = self.back_buffer_index * self.rtv_heap.descriptor_size;
        let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.rtv_heap.cpu_base.ptr + offset as usize,
        };
        (self.swap_buffers[self.back_buffer_index as usize], handle)
    }
}

impl DescriptorHeap {
    fn new(
        device: WeakPtr<ID3D12Device2>,
        capacity: u32,
        htype: D3D12_DESCRIPTOR_HEAP_TYPE,
        flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
    ) -> Self {
        let heap = {
            let mut rheap: *mut ID3D12DescriptorHeap = ptr::null_mut();
            vhr!(device.CreateDescriptorHeap(
                &D3D12_DESCRIPTOR_HEAP_DESC {
                    NumDescriptors: capacity,
                    Type: htype,
                    Flags: flags,
                    NodeMask: 0,
                },
                &ID3D12DescriptorHeap::uuidof(),
                &mut rheap as *mut *mut _ as *mut *mut c_void,
            ));
            WeakPtr::from_raw(rheap)
        };
        let (cpu_base, gpu_base) = unsafe {
            (
                heap.GetCPUDescriptorHandleForHeapStart(),
                if flags == D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE {
                    heap.GetGPUDescriptorHandleForHeapStart()
                } else {
                    D3D12_GPU_DESCRIPTOR_HANDLE { ptr: 0 }
                },
            )
        };
        Self {
            cpu_base,
            gpu_base,
            capacity,
            heap,
            size: 0,
            descriptor_size: unsafe { device.GetDescriptorHandleIncrementSize(htype) },
        }
    }

    fn allocate_cpu_descriptors(&mut self, num: u32) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        assert!(self.cpu_base.ptr != 0);
        assert!(self.gpu_base.ptr == 0);
        assert!((self.size + num) < self.capacity);

        let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.cpu_base.ptr + (self.size as usize) * (self.descriptor_size as usize),
        };

        self.size += num;
        handle
    }

    fn allocate_gpu_descriptors(
        &mut self,
        num: u32,
    ) -> (D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_GPU_DESCRIPTOR_HANDLE) {
        assert!(self.cpu_base.ptr != 0);
        assert!(self.gpu_base.ptr != 0);
        assert!((self.size + num) < self.capacity);

        let cpu_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.cpu_base.ptr + (self.size as usize) * (self.descriptor_size as usize),
        };
        let gpu_handle = D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: self.gpu_base.ptr + (self.size as u64) * (self.descriptor_size as u64),
        };

        self.size += num;
        (cpu_handle, gpu_handle)
    }
}

fn calc_graphics_pipeline_hash(desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC) -> u64 {
    let mut hasher = DefaultHasher::new();

    hasher.write(unsafe {
        slice::from_raw_parts(desc.VS.pShaderBytecode as *const u8, desc.VS.BytecodeLength)
    });
    hasher.write(unsafe {
        slice::from_raw_parts(desc.PS.pShaderBytecode as *const u8, desc.PS.BytecodeLength)
    });

    hasher.write_i32(desc.BlendState.AlphaToCoverageEnable);
    hasher.write_i32(desc.BlendState.IndependentBlendEnable);
    for i in 0..8 {
        hasher.write_i32(desc.BlendState.RenderTarget[i].BlendEnable);
        hasher.write_i32(desc.BlendState.RenderTarget[i].LogicOpEnable);
        hasher.write_u32(desc.BlendState.RenderTarget[i].SrcBlend);
        hasher.write_u32(desc.BlendState.RenderTarget[i].DestBlend);
        hasher.write_u32(desc.BlendState.RenderTarget[i].BlendOp);
        hasher.write_u32(desc.BlendState.RenderTarget[i].SrcBlendAlpha);
        hasher.write_u32(desc.BlendState.RenderTarget[i].DestBlendAlpha);
        hasher.write_u32(desc.BlendState.RenderTarget[i].BlendOpAlpha);
        hasher.write_u32(desc.BlendState.RenderTarget[i].LogicOp);
        hasher.write_u8(desc.BlendState.RenderTarget[i].RenderTargetWriteMask);
    }

    hasher.write_u32(desc.SampleMask);

    hasher.write_u32(desc.RasterizerState.FillMode);
    hasher.write_u32(desc.RasterizerState.CullMode);
    hasher.write_i32(desc.RasterizerState.FrontCounterClockwise);
    hasher.write_i32(desc.RasterizerState.DepthBias);
    hasher.write_u32(desc.RasterizerState.DepthBiasClamp.to_bits());
    hasher.write_u32(desc.RasterizerState.SlopeScaledDepthBias.to_bits());
    hasher.write_i32(desc.RasterizerState.DepthClipEnable);
    hasher.write_i32(desc.RasterizerState.MultisampleEnable);
    hasher.write_i32(desc.RasterizerState.AntialiasedLineEnable);
    hasher.write_u32(desc.RasterizerState.ForcedSampleCount);
    hasher.write_u32(desc.RasterizerState.ConservativeRaster);

    hasher.write_i32(desc.DepthStencilState.DepthEnable);
    hasher.write_u32(desc.DepthStencilState.DepthWriteMask);
    hasher.write_u32(desc.DepthStencilState.DepthFunc);
    hasher.write_i32(desc.DepthStencilState.StencilEnable);
    hasher.write_u8(desc.DepthStencilState.StencilReadMask);
    hasher.write_u8(desc.DepthStencilState.StencilWriteMask);
    hasher.write_u32(desc.DepthStencilState.FrontFace.StencilFailOp);
    hasher.write_u32(desc.DepthStencilState.FrontFace.StencilDepthFailOp);
    hasher.write_u32(desc.DepthStencilState.FrontFace.StencilPassOp);
    hasher.write_u32(desc.DepthStencilState.FrontFace.StencilFunc);
    hasher.write_u32(desc.DepthStencilState.BackFace.StencilFailOp);
    hasher.write_u32(desc.DepthStencilState.BackFace.StencilDepthFailOp);
    hasher.write_u32(desc.DepthStencilState.BackFace.StencilPassOp);
    hasher.write_u32(desc.DepthStencilState.BackFace.StencilFunc);

    hasher.write_u32(desc.InputLayout.NumElements);
    for i in 0..desc.InputLayout.NumElements {
        let elem = unsafe { &*desc.InputLayout.pInputElementDescs.offset(i as isize) };

        hasher.write(unsafe { CStr::from_ptr(elem.SemanticName).to_bytes() });
        hasher.write_u32(elem.SemanticIndex);
        hasher.write_u32(elem.Format);
        hasher.write_u32(elem.InputSlot);
        hasher.write_u32(elem.AlignedByteOffset);
        hasher.write_u32(elem.InputSlotClass);
        hasher.write_u32(elem.InstanceDataStepRate);
    }

    hasher.write_u32(desc.IBStripCutValue);
    hasher.write_u32(desc.PrimitiveTopologyType);

    hasher.write_u32(desc.NumRenderTargets);
    for i in 0..8 {
        hasher.write_u32(desc.RTVFormats[i]);
    }

    hasher.write_u32(desc.DSVFormat);

    hasher.write_u32(desc.SampleDesc.Count);
    hasher.write_u32(desc.SampleDesc.Quality);

    hasher.finish()
}

fn calc_compute_pipeline_hash(desc: &D3D12_COMPUTE_PIPELINE_STATE_DESC) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(unsafe {
        slice::from_raw_parts(desc.CS.pShaderBytecode as *const u8, desc.CS.BytecodeLength)
    });
    hasher.finish()
}

impl GpuMemoryHeap {
    fn new(device: WeakPtr<ID3D12Device2>, capacity: u32, heap_type: D3D12_HEAP_TYPE) -> Self {
        // TODO(mziulek): Remove this limitation.
        assert!(heap_type == D3D12_HEAP_TYPE_UPLOAD);

        let heap = {
            let mut heap_raw: *mut ID3D12Resource = ptr::null_mut();
            vhr!(device.CreateCommittedResource(
                &Dx12HeapProperties::new(heap_type),
                D3D12_HEAP_FLAG_NONE,
                &Dx12ResourceDesc::buffer(capacity as u64),
                D3D12_RESOURCE_STATE_GENERIC_READ,
                ptr::null(),
                &ID3D12Resource::uuidof(),
                &mut heap_raw as *mut *mut _ as *mut *mut c_void
            ));
            WeakPtr::from_raw(heap_raw)
        };

        let mut cpu_base: *mut u8 = ptr::null_mut();
        vhr!(heap.Map(
            0,
            &D3D12_RANGE { Begin: 0, End: 0 },
            &mut cpu_base as *mut *mut _ as *mut *mut c_void
        ));

        let gpu_base = unsafe { heap.GetGPUVirtualAddress() };

        Self {
            heap,
            cpu_base,
            gpu_base,
            size: 0,
            capacity,
        }
    }

    fn allocate(&mut self, mut size: u32) -> (*mut c_void, D3D12_GPU_VIRTUAL_ADDRESS) {
        assert!(size > 0);

        if (size & 0xff) != 0 {
            size = (size + 255) & !0xff;
        }

        if (self.size + size) >= self.capacity {
            return (ptr::null_mut(), 0);
        }

        let cpu_addr = unsafe { self.cpu_base.offset(self.size as isize) as *mut c_void };
        let gpu_addr = self.gpu_base + self.size as u64;

        self.size += size;
        (cpu_addr, gpu_addr)
    }
}
