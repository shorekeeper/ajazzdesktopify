use ash::vk;
use std::ffi::CStr;
use crate::ui::draw::{DrawCmd, Vertex};
use crate::ui::text::{ATLAS_W, ATLAS_H};
use crate::ui::theme as t;

const MAX_FRAMES: usize = 2;
const VERT_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.vert.spv"));
const FRAG_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.frag.spv"));

struct GpuBuf { buf: vk::Buffer, mem: vk::DeviceMemory, ptr: *mut u8, size: u64 }
impl GpuBuf { const EMPTY: Self = Self { buf: vk::Buffer::null(), mem: vk::DeviceMemory::null(), ptr: std::ptr::null_mut(), size: 0 }; }

pub struct Vk {
    #[allow(dead_code)] entry: ash::Entry,
    instance: ash::Instance,
    surface_fn: ash::khr::surface::Instance,
    swap_fn: ash::khr::swapchain::Device,
    dev: ash::Device,
    pdev: vk::PhysicalDevice,
    queue: vk::Queue,
    #[allow(dead_code)] qfam: u32,
    surface: vk::SurfaceKHR,
    sc: vk::SwapchainKHR,
    sc_views: Vec<vk::ImageView>,
    sc_fmt: vk::Format,
    sc_ext: vk::Extent2D,
    fbs: Vec<vk::Framebuffer>,
    rp: vk::RenderPass,
    pl: vk::Pipeline,
    pl_layout: vk::PipelineLayout,
    ds_layout: vk::DescriptorSetLayout,
    ds_pool: vk::DescriptorPool,
    ds: vk::DescriptorSet,
    pool: vk::CommandPool,
    cmds: [vk::CommandBuffer; MAX_FRAMES],
    sem_avail: [vk::Semaphore; MAX_FRAMES],
    sem_done:  [vk::Semaphore; MAX_FRAMES],
    fences:    [vk::Fence; MAX_FRAMES],
    frame: usize,
    vb: [GpuBuf; MAX_FRAMES],
    ib: [GpuBuf; MAX_FRAMES],
    atlas_img: vk::Image,
    atlas_mem: vk::DeviceMemory,
    atlas_view: vk::ImageView,
    atlas_sampler: vk::Sampler,
    staging: GpuBuf,
    mem_props: vk::PhysicalDeviceMemoryProperties,
}

fn find_mem(props: &vk::PhysicalDeviceMemoryProperties, filter: u32, flags: vk::MemoryPropertyFlags) -> u32 {
    for i in 0..props.memory_type_count {
        if filter & (1 << i) != 0 && props.memory_types[i as usize].property_flags.contains(flags) { return i; }
    }
    panic!("No suitable memory type");
}

impl Vk {
    pub fn new(hwnd: isize, hinstance: isize, w: u32, h: u32) -> Self { unsafe { Self::init(hwnd, hinstance, w, h) } }

    unsafe fn init(hwnd: isize, hinstance: isize, w: u32, h: u32) -> Self {
        let entry = ash::Entry::load().expect("Vulkan not available");
        let app_info = vk::ApplicationInfo::default()
            .application_name(CStr::from_bytes_with_nul_unchecked(b"AK680\0"))
            .api_version(vk::make_api_version(0, 1, 2, 0));
        let exts = [ash::khr::surface::NAME.as_ptr(), ash::khr::win32_surface::NAME.as_ptr()];
        let ci = vk::InstanceCreateInfo::default().application_info(&app_info).enabled_extension_names(&exts);
        let instance = entry.create_instance(&ci, None).expect("vkCreateInstance");

        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        let win32_fn = ash::khr::win32_surface::Instance::new(&entry, &instance);
        let sci = vk::Win32SurfaceCreateInfoKHR::default().hwnd(hwnd as vk::HWND).hinstance(hinstance as vk::HINSTANCE);
        let surface = win32_fn.create_win32_surface(&sci, None).expect("create_win32_surface");

        let pdevs = instance.enumerate_physical_devices().unwrap();
        let (pdev, qfam) = pdevs.iter().find_map(|&pd| {
            let qp = instance.get_physical_device_queue_family_properties(pd);
            for (i, q) in qp.iter().enumerate() {
                if q.queue_flags.contains(vk::QueueFlags::GRAPHICS) && surface_fn.get_physical_device_surface_support(pd, i as u32, surface).unwrap_or(false) {
                    return Some((pd, i as u32));
                }
            }
            None
        }).expect("No suitable GPU");
        let mem_props = instance.get_physical_device_memory_properties(pdev);

        let prio = [1.0f32];
        let qci = [vk::DeviceQueueCreateInfo::default().queue_family_index(qfam).queue_priorities(&prio)];
        let dev_exts = [ash::khr::swapchain::NAME.as_ptr()];
        let dci = vk::DeviceCreateInfo::default().queue_create_infos(&qci).enabled_extension_names(&dev_exts);
        let dev = instance.create_device(pdev, &dci, None).expect("vkCreateDevice");
        let queue = dev.get_device_queue(qfam, 0);
        let swap_fn = ash::khr::swapchain::Device::new(&instance, &dev);

        let pci = vk::CommandPoolCreateInfo::default().queue_family_index(qfam).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let pool = dev.create_command_pool(&pci, None).unwrap();

        let att = [vk::AttachmentDescription::default().format(vk::Format::B8G8R8A8_UNORM).samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
        let aref = [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let sub = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&aref)];
        let dep = [vk::SubpassDependency::default().src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT).dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
        let rpci = vk::RenderPassCreateInfo::default().attachments(&att).subpasses(&sub).dependencies(&dep);
        let rp = dev.create_render_pass(&rpci, None).unwrap();

        let bindings = [vk::DescriptorSetLayoutBinding::default().binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let dslci = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let ds_layout = dev.create_descriptor_set_layout(&dslci, None).unwrap();

        let pc_range = [vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX).offset(0).size(8)];
        let plci = vk::PipelineLayoutCreateInfo::default().set_layouts(std::slice::from_ref(&ds_layout)).push_constant_ranges(&pc_range);
        let pl_layout = dev.create_pipeline_layout(&plci, None).unwrap();

        let mk_mod = |code: &[u8]| -> vk::ShaderModule {
            let aligned: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes([c[0],c[1],c[2],c[3]])).collect();
            dev.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&aligned), None).unwrap()
        };
        let vs = mk_mod(VERT_SPV);
        let fs = mk_mod(FRAG_SPV);
        let entry_name = CStr::from_bytes_with_nul_unchecked(b"main\0");
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(entry_name),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(entry_name),
        ];

        let bind = [vk::VertexInputBindingDescription::default().binding(0).stride(std::mem::size_of::<Vertex>() as u32).input_rate(vk::VertexInputRate::VERTEX)];
        let attrs = [
            vk::VertexInputAttributeDescription::default().location(0).binding(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().location(1).binding(0).format(vk::Format::R32G32_SFLOAT).offset(8),
            vk::VertexInputAttributeDescription::default().location(2).binding(0).format(vk::Format::R32G32B32A32_SFLOAT).offset(16),
        ];
        let vis = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bind).vertex_attribute_descriptions(&attrs);
        let ias = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend_att = [vk::PipelineColorBlendAttachmentState::default().blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE).dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cbs = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att);
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let ds_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);

        let gpci = [vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&vis).input_assembly_state(&ias)
            .viewport_state(&vps).rasterization_state(&rs).multisample_state(&ms).color_blend_state(&cbs).dynamic_state(&ds_state)
            .layout(pl_layout).render_pass(rp).subpass(0)];
        let pl = dev.create_graphics_pipelines(vk::PipelineCache::null(), &gpci, None).unwrap()[0];
        dev.destroy_shader_module(vs, None);
        dev.destroy_shader_module(fs, None);

        let pool_sizes = [vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1)];
        let dpci = vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&pool_sizes);
        let ds_pool = dev.create_descriptor_pool(&dpci, None).unwrap();
        let dsai = vk::DescriptorSetAllocateInfo::default().descriptor_pool(ds_pool).set_layouts(std::slice::from_ref(&ds_layout));
        let ds = dev.allocate_descriptor_sets(&dsai).unwrap()[0];

        let sci_sem = vk::SemaphoreCreateInfo::default();
        let fci = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let sem_avail = [dev.create_semaphore(&sci_sem, None).unwrap(), dev.create_semaphore(&sci_sem, None).unwrap()];
        let sem_done  = [dev.create_semaphore(&sci_sem, None).unwrap(), dev.create_semaphore(&sci_sem, None).unwrap()];
        let fences    = [dev.create_fence(&fci, None).unwrap(), dev.create_fence(&fci, None).unwrap()];

        let cbai = vk::CommandBufferAllocateInfo::default().command_pool(pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(MAX_FRAMES as u32);
        let cbs_vec = dev.allocate_command_buffers(&cbai).unwrap();
        let cmds = [cbs_vec[0], cbs_vec[1]];

        let mk_dyn_buf = |sz: u64| -> GpuBuf {
            let bci = vk::BufferCreateInfo::default().size(sz).usage(vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER).sharing_mode(vk::SharingMode::EXCLUSIVE);
            let buf = dev.create_buffer(&bci, None).unwrap();
            let req = dev.get_buffer_memory_requirements(buf);
            let mt = find_mem(&mem_props, req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT);
            let ai = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(mt);
            let mem = dev.allocate_memory(&ai, None).unwrap();
            dev.bind_buffer_memory(buf, mem, 0).unwrap();
            let ptr = dev.map_memory(mem, 0, sz, vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
            GpuBuf { buf, mem, ptr, size: sz }
        };
        let vb = [mk_dyn_buf(4 * 1024 * 1024), mk_dyn_buf(4 * 1024 * 1024)];
        let ib = [mk_dyn_buf(2 * 1024 * 1024), mk_dyn_buf(2 * 1024 * 1024)];

        let ici = vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(vk::Format::R8_UNORM)
            .extent(vk::Extent3D { width: ATLAS_W, height: ATLAS_H, depth: 1 }).mip_levels(1).array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1).tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST);
        let atlas_img = dev.create_image(&ici, None).unwrap();
        let req = dev.get_image_memory_requirements(atlas_img);
        let mt = find_mem(&mem_props, req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL);
        let ai = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(mt);
        let atlas_mem = dev.allocate_memory(&ai, None).unwrap();
        dev.bind_image_memory(atlas_img, atlas_mem, 0).unwrap();

        let ivci = vk::ImageViewCreateInfo::default().image(atlas_img).view_type(vk::ImageViewType::TYPE_2D).format(vk::Format::R8_UNORM)
            .components(vk::ComponentMapping { r: vk::ComponentSwizzle::R, g: vk::ComponentSwizzle::R, b: vk::ComponentSwizzle::R, a: vk::ComponentSwizzle::R })
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let atlas_view = dev.create_image_view(&ivci, None).unwrap();

        let sampler_ci = vk::SamplerCreateInfo::default().mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE).address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE);
        let atlas_sampler = dev.create_sampler(&sampler_ci, None).unwrap();

        let staging = mk_dyn_buf((ATLAS_W * ATLAS_H) as u64);

        let img_info = [vk::DescriptorImageInfo::default().image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).image_view(atlas_view).sampler(atlas_sampler)];
        let writes = [vk::WriteDescriptorSet::default().dst_set(ds).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&img_info)];
        dev.update_descriptor_sets(&writes, &[]);

        let mut ctx = Self {
            entry, instance, surface_fn, swap_fn, dev, pdev, queue, qfam, surface,
            sc: vk::SwapchainKHR::null(), sc_views: Vec::new(), sc_fmt: vk::Format::B8G8R8A8_UNORM,
            sc_ext: vk::Extent2D { width: w, height: h }, fbs: Vec::new(),
            rp, pl, pl_layout, ds_layout, ds_pool, ds, pool, cmds, sem_avail, sem_done, fences, frame: 0,
            vb, ib, atlas_img, atlas_mem, atlas_view, atlas_sampler, staging, mem_props,
        };
        ctx.create_swapchain(w, h);
        ctx
    }

    unsafe fn create_swapchain(&mut self, w: u32, h: u32) {
        let caps = self.surface_fn.get_physical_device_surface_capabilities(self.pdev, self.surface).unwrap();
        let extent = vk::Extent2D {
            width:  w.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
            height: h.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
        };
        let img_count = (caps.min_image_count + 1).min(if caps.max_image_count > 0 { caps.max_image_count } else { u32::MAX });
        let sci = vk::SwapchainCreateInfoKHR::default().surface(self.surface).min_image_count(img_count)
            .image_format(vk::Format::B8G8R8A8_UNORM).image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(extent).image_array_layers(1).image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE).pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE).present_mode(vk::PresentModeKHR::FIFO)
            .old_swapchain(self.sc);
        let new_sc = self.swap_fn.create_swapchain(&sci, None).unwrap();
        if self.sc != vk::SwapchainKHR::null() { self.destroy_swapchain_views(); self.swap_fn.destroy_swapchain(self.sc, None); }
        self.sc = new_sc;
        self.sc_ext = extent;
        let images = self.swap_fn.get_swapchain_images(self.sc).unwrap();
        self.sc_views = images.iter().map(|&img| {
            let ivci = vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(self.sc_fmt)
                .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
            self.dev.create_image_view(&ivci, None).unwrap()
        }).collect();
        self.fbs = self.sc_views.iter().map(|&v| {
            let atts = [v];
            let fbci = vk::FramebufferCreateInfo::default().render_pass(self.rp).attachments(&atts).width(extent.width).height(extent.height).layers(1);
            self.dev.create_framebuffer(&fbci, None).unwrap()
        }).collect();
    }

    unsafe fn destroy_swapchain_views(&mut self) {
        for &fb in &self.fbs { self.dev.destroy_framebuffer(fb, None); }
        for &v in &self.sc_views { self.dev.destroy_image_view(v, None); }
        self.fbs.clear(); self.sc_views.clear();
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        unsafe { self.dev.device_wait_idle().unwrap(); self.create_swapchain(w, h); }
    }

    pub fn upload_atlas(&mut self, data: &[u8]) {
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.staging.ptr, data.len());
            let cmd = self.one_time_begin();
            let range = vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 };
            let bar = [vk::ImageMemoryBarrier::default().image(self.atlas_img).subresource_range(range)
                .old_layout(vk::ImageLayout::UNDEFINED).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)];
            self.dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &bar);
            let region = [vk::BufferImageCopy {
                image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
                image_extent: vk::Extent3D { width: ATLAS_W, height: ATLAS_H, depth: 1 }, ..Default::default()
            }];
            self.dev.cmd_copy_buffer_to_image(cmd, self.staging.buf, self.atlas_img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &region);
            let bar2 = [vk::ImageMemoryBarrier::default().image(self.atlas_img).subresource_range(range)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)];
            self.dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &bar2);
            self.one_time_end(cmd);
        }
    }

    unsafe fn one_time_begin(&self) -> vk::CommandBuffer {
        let ai = vk::CommandBufferAllocateInfo::default().command_pool(self.pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
        let cmd = self.dev.allocate_command_buffers(&ai).unwrap()[0];
        self.dev.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap();
        cmd
    }

    unsafe fn one_time_end(&self, cmd: vk::CommandBuffer) {
        self.dev.end_command_buffer(cmd).unwrap();
        let si = [vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd))];
        self.dev.queue_submit(self.queue, &si, vk::Fence::null()).unwrap();
        self.dev.queue_wait_idle(self.queue).unwrap();
        self.dev.free_command_buffers(self.pool, &[cmd]);
    }

    pub fn render(&mut self, verts: &[Vertex], idxs: &[u32], cmds: &[DrawCmd]) {
        if self.sc_ext.width == 0 || self.sc_ext.height == 0 { return; }
        unsafe { self.render_inner(verts, idxs, cmds) }
    }

    unsafe fn render_inner(&mut self, verts: &[Vertex], idxs: &[u32], draw_cmds: &[DrawCmd]) {
        let f = self.frame;
        self.dev.wait_for_fences(&[self.fences[f]], true, u64::MAX).unwrap();
        let acq = self.swap_fn.acquire_next_image(self.sc, u64::MAX, self.sem_avail[f], vk::Fence::null());
        let img_idx = match acq {
            Ok((idx, _)) => idx,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => { self.resize(self.sc_ext.width, self.sc_ext.height); return; }
            Err(e) => panic!("acquire: {e}"),
        };
        self.dev.reset_fences(&[self.fences[f]]).unwrap();

        let vb_bytes = std::mem::size_of_val(verts);
        let ib_bytes = std::mem::size_of_val(idxs);

        // Prevent writing past mapped GPU memory
        if vb_bytes as u64 > self.vb[f].size || ib_bytes as u64 > self.ib[f].size {
            log::error!(
                "Draw data exceeds GPU buffer: vtx {vb_bytes} / {}, idx {ib_bytes} / {}",
                self.vb[f].size, self.ib[f].size
            );
            return;
        }

        if vb_bytes > 0 { std::ptr::copy_nonoverlapping(verts.as_ptr() as *const u8, self.vb[f].ptr, vb_bytes); }
        if ib_bytes > 0 { std::ptr::copy_nonoverlapping(idxs.as_ptr()  as *const u8, self.ib[f].ptr, ib_bytes); }

        let cmd = self.cmds[f];
        self.dev.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty()).unwrap();
        self.dev.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default()).unwrap();

        let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: [t::BG_BASE[0], t::BG_BASE[1], t::BG_BASE[2], 1.0] } }];
        let rpbi = vk::RenderPassBeginInfo::default().render_pass(self.rp).framebuffer(self.fbs[img_idx as usize])
            .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.sc_ext }).clear_values(&clear);
        self.dev.cmd_begin_render_pass(cmd, &rpbi, vk::SubpassContents::INLINE);
        self.dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pl);

        let vp = [vk::Viewport { x: 0.0, y: 0.0, width: self.sc_ext.width as f32, height: self.sc_ext.height as f32, min_depth: 0.0, max_depth: 1.0 }];
        self.dev.cmd_set_viewport(cmd, 0, &vp);

        let pc = [self.sc_ext.width as f32, self.sc_ext.height as f32];
        self.dev.cmd_push_constants(cmd, self.pl_layout, vk::ShaderStageFlags::VERTEX, 0,
            std::slice::from_raw_parts(pc.as_ptr() as *const u8, 8));

        self.dev.cmd_bind_vertex_buffers(cmd, 0, &[self.vb[f].buf], &[0]);
        self.dev.cmd_bind_index_buffer(cmd, self.ib[f].buf, 0, vk::IndexType::UINT32);
        self.dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.pl_layout, 0, &[self.ds], &[]);

        for dc in draw_cmds {
            let sc = vk::Rect2D { offset: vk::Offset2D { x: dc.clip[0], y: dc.clip[1] },
                extent: vk::Extent2D { width: dc.clip[2] as u32, height: dc.clip[3] as u32 } };
            self.dev.cmd_set_scissor(cmd, 0, &[sc]);
            self.dev.cmd_draw_indexed(cmd, dc.index_count, 1, dc.index_offset, 0, 0);
        }

        self.dev.cmd_end_render_pass(cmd);
        self.dev.end_command_buffer(cmd).unwrap();

        let wait_sem = [self.sem_avail[f]];
        let sig_sem = [self.sem_done[f]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let si = [vk::SubmitInfo::default().wait_semaphores(&wait_sem).wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&cmd)).signal_semaphores(&sig_sem)];
        self.dev.queue_submit(self.queue, &si, self.fences[f]).unwrap();

        let scs = [self.sc]; let idxs_p = [img_idx];
        let pi = vk::PresentInfoKHR::default().wait_semaphores(&sig_sem).swapchains(&scs).image_indices(&idxs_p);
        let _ = self.swap_fn.queue_present(self.queue, &pi);
        self.frame = (f + 1) % MAX_FRAMES;
    }
}

impl Drop for Vk {
    fn drop(&mut self) {
        unsafe {
            self.dev.device_wait_idle().unwrap();
            let free_buf = |d: &ash::Device, b: &GpuBuf| { if b.buf != vk::Buffer::null() { d.destroy_buffer(b.buf, None); d.free_memory(b.mem, None); } };
            for i in 0..MAX_FRAMES { free_buf(&self.dev, &self.vb[i]); free_buf(&self.dev, &self.ib[i]); }
            free_buf(&self.dev, &self.staging);
            self.dev.destroy_sampler(self.atlas_sampler, None);
            self.dev.destroy_image_view(self.atlas_view, None);
            self.dev.destroy_image(self.atlas_img, None);
            self.dev.free_memory(self.atlas_mem, None);
            for i in 0..MAX_FRAMES { self.dev.destroy_semaphore(self.sem_avail[i], None); self.dev.destroy_semaphore(self.sem_done[i], None); self.dev.destroy_fence(self.fences[i], None); }
            self.dev.destroy_command_pool(self.pool, None);
            self.destroy_swapchain_views();
            self.swap_fn.destroy_swapchain(self.sc, None);
            self.dev.destroy_pipeline(self.pl, None);
            self.dev.destroy_pipeline_layout(self.pl_layout, None);
            self.dev.destroy_render_pass(self.rp, None);
            self.dev.destroy_descriptor_pool(self.ds_pool, None);
            self.dev.destroy_descriptor_set_layout(self.ds_layout, None);
            self.dev.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}