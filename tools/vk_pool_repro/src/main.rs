//! Minimal isolated Vulkan reproduction (issue #89, agent Y): create a descriptor pool with the parameters wgpu-hal 30.0.1
//! uses for an empty bind group layout, and variants. Usage: vkrepro <case> [--ext]
//! Cases: empty (flags FREE_DESCRIPTOR_SET, maxSets 64, poolSizeCount 0, non-null pPoolSizes), empty-null (pPoolSizes null),
//!        empty-noflags (flags 0), one (poolSizeCount 1: UNIFORM_BUFFER 1), none (only create the device).
use ash::vk;
use std::ffi::CStr;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let case = args.get(1).map(String::as_str).unwrap_or("empty");
    let entry = unsafe { ash::Entry::load() }.expect("load vulkan");
    let app = vk::ApplicationInfo::default().api_version(vk::make_api_version(0, 1, 3, 0));
    let inst_info = vk::InstanceCreateInfo::default().application_info(&app);
    let instance = unsafe { entry.create_instance(&inst_info, None) }.expect("instance");
    let pds = unsafe { instance.enumerate_physical_devices() }.expect("pds");
    let pd = pds[0];
    let props = unsafe { instance.get_physical_device_properties(pd) };
    let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    println!(
        "device {name} api {}.{}.{} driver 0x{:x} vendor 0x{:x}",
        vk::api_version_major(props.api_version),
        vk::api_version_minor(props.api_version),
        vk::api_version_patch(props.api_version),
        props.driver_version,
        props.vendor_id
    );
    let prio = [1.0f32];
    let qf = unsafe { instance.get_physical_device_queue_family_properties(pd) }
        .iter()
        .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .unwrap() as u32;
    let qi = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(qf)
        .queue_priorities(&prio)];
    let dev_info = vk::DeviceCreateInfo::default().queue_create_infos(&qi);
    let device = unsafe { instance.create_device(pd, &dev_info, None) }.expect("device");
    println!("vkCreateDevice ok");
    let sizes_one = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::UNIFORM_BUFFER,
        descriptor_count: 1,
    }];
    let sizes_none: [vk::DescriptorPoolSize; 0] = [];
    let (flags, sizes): (vk::DescriptorPoolCreateFlags, &[vk::DescriptorPoolSize]) = match case {
        "empty" | "empty-null" => (
            vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
            &sizes_none,
        ),
        "empty-noflags" => (vk::DescriptorPoolCreateFlags::empty(), &sizes_none),
        "one" => (
            vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
            &sizes_one,
        ),
        _ => {
            println!("case none: done");
            return;
        }
    };
    let mut info = vk::DescriptorPoolCreateInfo::default()
        .flags(flags)
        .max_sets(64)
        .pool_sizes(sizes);
    if case == "empty-null" {
        info.p_pool_sizes = std::ptr::null();
    }
    println!("VkDescriptorPoolCreateInfo sType {:?} pNext {:?} flags {:?} maxSets {} poolSizeCount {} pPoolSizes {:?}",
        info.s_type, info.p_next, info.flags, info.max_sets, info.pool_size_count, info.p_pool_sizes);
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    let r = unsafe { device.create_descriptor_pool(&info, None) };
    println!("vkCreateDescriptorPool -> {:?}", r.as_ref().map(|_| "ok"));
    if let Ok(p) = r {
        unsafe { device.destroy_descriptor_pool(p, None) };
    }
    unsafe {
        device.destroy_device(None);
        instance.destroy_instance(None);
    }
    println!("done");
}
