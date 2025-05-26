#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/uaccess.h>
#include <linux/version.h>
#include <linux/slab.h>

unsigned long my_copy_from_user(void *to, const void __user *from, unsigned long n)
{
    unsigned long not_copied = 0;
    unsigned char *dst = (unsigned char *)to;
    const unsigned char __user *src = (const unsigned char __user *)from;
    
    // 基本地址有效性检查
    if (!access_ok(from, n))
        return n;
    
    // 简单逐字节复制实现
    while (n > 0) {
        unsigned char c;
        if (__get_user(c, src)) {
            not_copied = n;
            break;
        }
        *dst++ = c;
        src++;
        n--;
    }
    
    // 如果复制失败，手动用零填充剩余部分
    if (not_copied > 0) {
        unsigned char *fill_dst = dst;
        unsigned long remain = not_copied;
        
        // 手动实现memset功能
        while (remain > 0) {
            *fill_dst++ = 0;
            remain--;
        }
    }
    
    return not_copied;
}

// 导出函数
EXPORT_SYMBOL(my_copy_from_user);

static int __init copy_shim_init(void)
{
    //printk(KERN_INFO "custom copy_from_user shim module loaded\n");
    return 0;
}

static void __exit copy_shim_exit(void)
{
    //printk(KERN_INFO "custom copy_from_user shim module unloaded\n");
}

MODULE_LICENSE("GPL");
MODULE_AUTHOR("Pro");
MODULE_DESCRIPTION("copy_from_user shim");

module_init(copy_shim_init);
module_exit(copy_shim_exit);