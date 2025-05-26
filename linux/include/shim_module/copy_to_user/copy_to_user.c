#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/uaccess.h>
#include <linux/version.h>


unsigned long my_copy_to_user(void __user *to, const void *from, unsigned long n)
{
    unsigned long not_copied = 0;
    unsigned char __user *dst = (unsigned char __user *)to;
    const unsigned char *src = (const unsigned char *)from;
    
    // 基本地址有效性检查 - 仅保留这个最低限度的检查
    if (!access_ok(to, n))
        return n;
    
    // 简单逐字节复制实现
    while (n > 0) {
        if (__put_user(*src, dst)) {
            not_copied = n;
            break;
        }
        dst++;
        src++;
        n--;
    }
    
    return not_copied;
}

EXPORT_SYMBOL(my_copy_to_user);

static int __init copy_to_user_shim_init(void)
{
    //printk(KERN_INFO "copy_to_user shim module loaded\n");
    return 0;
}

static void __exit copy_to_user_shim_exit(void)
{
    //printk(KERN_INFO "copy_to_user shim module unloaded\n");
}

MODULE_LICENSE("GPL");
MODULE_AUTHOR("Pro");
MODULE_DESCRIPTION("copy_to_user shim");

module_init(copy_to_user_shim_init);
module_exit(copy_to_user_shim_exit);