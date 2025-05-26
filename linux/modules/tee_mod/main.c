#include "linux/mmzone.h"
#include "linux/version.h"
#include <asm-generic/memory_model.h>
#include <linux/anon_inodes.h>
#include <linux/fs.h>
#if LINUX_VERSION_CODE > KERNEL_VERSION(6, 0, 0)
#include <linux/gfp_types.h>
#else
#include <linux/gfp.h>
#endif
#include <linux/kernel.h>
#include <linux/list.h>
#include <linux/miscdevice.h>
#include <linux/mm.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/printk.h>
#include <linux/slab.h>
#include <linux/types.h>
#include <linux/uaccess.h>

#define IOCTL_ALLOC_MEM _IOW('k', 1, struct user_arg)

#ifndef MAX_ORDER
#define MAX_ORDER MAX_PAGE_ORDER
#endif

MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("TEE control");

struct mem_block {
  struct page *page;
  unsigned int order;
  struct list_head list;
  struct page *extra;
};

struct mem_region {
  struct list_head blocks;
  size_t size;
};

struct user_arg {
  size_t size;
  int fd;
};

// static LIST_HEAD(region_list);
// static DEFINE_MUTEX(region_lock);
static atomic_t region_id = ATOMIC_INIT(0);

static size_t fixed_size(size_t size) {
  size_t pages_needed = (size + PAGE_SIZE - 1) >> PAGE_SHIFT;
  if (pages_needed < (1UL << MAX_ORDER)) {
    pages_needed = 1UL << min(fls(pages_needed - 1), MAX_ORDER);
  }

  size = PAGE_SIZE * pages_needed;

  return size;
}

static void free_region(struct mem_region *region) {
  struct mem_block *block, *tmp;

  list_for_each_entry_safe(block, tmp, &region->blocks, list) {
    __free_pages(block->page, block->order);
    if (block->extra != NULL)
      __free_pages(block->extra, 0);
    list_del(&block->list);
    kfree(block);
  }

  pr_info("[teectl] free region, size: %zu\n", region->size);
}

static int mem_release(struct inode *inode, struct file *filp) {
  struct mem_region *region = filp->private_data;

  free_region(region);
  kfree(region);

  return 0;
}

static int alloc_blocks(struct mem_region *region, size_t size) {
  size_t pages_needed = (size + PAGE_SIZE - 1) >> PAGE_SHIFT;

  pr_info("[teectl] %zu pages needed\n", pages_needed);

  while (pages_needed > 0) {
    unsigned int order = min(fls(pages_needed) - 1, MAX_ORDER);
    struct mem_block *block;
    struct page *page;

    while (order > 0 && !(page = alloc_pages(GFP_HIGHUSER, order)))
      order--;

    if (!page)
      return -ENOMEM;

    block = kmalloc(sizeof(*block), GFP_KERNEL);
    if (!block) {
      __free_pages(page, order);
      return -ENOMEM;
    }

    block->page = page;
    block->order = order;
    list_add_tail(&block->list, &region->blocks);

    if (pages_needed > (1 << order))
      pages_needed -= (1 << order);
    else
      pages_needed = 0;
  }
  return 0;
}

static int mem_mmap(struct file *filp, struct vm_area_struct *vma) {
  struct mem_region *region = filp->private_data;
  unsigned long addr = vma->vm_start;
  int ret;
  struct mem_block *block;

  ret = alloc_blocks(region, region->size);
  if (ret) {
    return ret;
  }

  list_for_each_entry(block, &region->blocks, list) {
    unsigned long pfn = page_to_pfn(block->page);
    size_t block_size = PAGE_SIZE << block->order;

    // pr_info("pfn: %lu, order: %d, block_size: %zu", pfn, block->order,
    //         block_size);

    ret = remap_pfn_range(vma, addr, pfn, block_size - PAGE_SIZE, vma->vm_page_prot);
    if (ret) {
      return ret;
    }
    block->extra = alloc_pages(GFP_HIGHUSER, 0);
    pfn = page_to_pfn(block->extra);
    ret =
        remap_pfn_range(vma, addr + block_size - PAGE_SIZE, pfn, PAGE_SIZE, vma->vm_page_prot);
    if (ret) {
      return ret;
    }
    addr += block_size;
  }

  return 0;
}

static const struct file_operations region_fops = {
    .owner = THIS_MODULE,
    .mmap = mem_mmap,
    .release = mem_release,
};

static long ioctl_handler(struct file *filp, unsigned int cmd,
                          unsigned long arg) {
  struct user_arg uarg;

  if (cmd != IOCTL_ALLOC_MEM)
    return -EINVAL;

  if (copy_from_user(&uarg, (void __user *)arg, sizeof(uarg)))
    return -EFAULT;

  struct mem_region *region = kzalloc(sizeof(*region), GFP_KERNEL);
  if (!region)
    return -ENOMEM;

  INIT_LIST_HEAD(&region->blocks);
  uarg.size = fixed_size(uarg.size);
  region->size = uarg.size;

  // char name[16];
  // snprintf(name, sizeof(name), "cmem-%d", atomic_inc_return(&region_id));

  int fd = anon_inode_getfd("cmem", &region_fops, region, O_RDWR);
  if (fd < 0) {
    pr_err("[teectl] Failed to get anon inode fd");
    kfree(region);
    return fd;
  }

  uarg.fd = fd;

  if (copy_to_user((void __user *)arg, &uarg, sizeof(uarg))) {
    /* Cleanup will happen through release() */
    return -EFAULT;
  }

  return 0;
}

static struct file_operations fops = {
    .unlocked_ioctl = ioctl_handler,
};

static struct miscdevice teectl_dev = {
    .minor = MISC_DYNAMIC_MINOR,
    .name = "teectl",
    .fops = &fops,
};

static int __init teectl_init(void) { return misc_register(&teectl_dev); }

static void __exit teectl_exit(void) { misc_deregister(&teectl_dev); }

module_init(teectl_init);
module_exit(teectl_exit);