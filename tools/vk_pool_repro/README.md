# vk_pool_repro

Minimal program for issue #89 (not part of the product crates). It makes a Vulkan device with no features and then one descriptor pool. The case is required; an unknown case or an extra argument prints the usage and exits with status 2:

```
cd tools/vk_pool_repro && cargo run --release -- <case>
```

| case | `vkCreateDescriptorPool` call (`maxSets` 64, `pNext` null) |
| --- | --- |
| `none` | no pool, device only |
| `empty` | FREE_DESCRIPTOR_SET, `poolSizeCount` 0, `pPoolSizes` non-null (what wgpu-hal 30.0.1 sends for an empty layout) |
| `empty-null` | the same with `pPoolSizes` null |
| `empty-noflags` | flags 0, `poolSizeCount` 0 |
| `one` | FREE_DESCRIPTOR_SET, one pool size (UNIFORM_BUFFER 1) |

Results on the Intel UHD (driver 31.0.101.2140) and on an NVIDIA RTX 3060 Ti: `vendor/wgpu-hal/VENDOR.md`. The program prints the
`VkDescriptorPoolCreateInfo` fields before the call.
