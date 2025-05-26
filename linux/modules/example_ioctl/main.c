#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/miscdevice.h>
#include "../../include/lde.h"

#define IOCTL_UPDATE_BUF _IOW('k', 1, struct user_arg)

MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Ioctl example");

struct user_arg {
    char buf[128];
};

static long ioctl(struct file *file, unsigned int cmd, unsigned long arg)
{
    struct user_arg user_arg;
    int rc;
    
    pr_info("command: %x", cmd);

    switch (cmd) {
    case IOCTL_UPDATE_BUF:
        if (!arg)
            return -EFAULT;

        ecall_eenter(rc);

        // 1. 获取用户数据
        if (copy_from_user(&user_arg, (void __user *)arg, sizeof(user_arg))) {
            ecall_eexit(rc);
            return -EFAULT;
        }


        // 2. 添加后缀（确保不溢出）
        size_t len = strlen(user_arg.buf);
        if (len + sizeof(" from kernel") > sizeof(user_arg.buf)) {
            ecall_eexit(rc);
            return -EINVAL;
        }
        strncat(user_arg.buf, " from kernel", sizeof(user_arg.buf) - len - 1);

        // 3. 返回修改后的数据
        if (copy_to_user((void __user *)arg, &user_arg, sizeof(user_arg))) {
            ecall_eexit(rc);
            return -EFAULT;
        }


        ecall_eexit(rc);
        break;

    default:
        return -EINVAL;
    }
    return 0;
}

static const struct file_operations fops = {
    .unlocked_ioctl = ioctl,
};

static struct miscdevice example_device = {
    .minor = MISC_DYNAMIC_MINOR,
    .name = "ioctl_example",
    .fops = &fops,
};

static int __init example_init(void)
{
    return misc_register(&example_device);
}

static void __exit example_exit(void)
{
    misc_deregister(&example_device);
}

module_init(example_init);
module_exit(example_exit);