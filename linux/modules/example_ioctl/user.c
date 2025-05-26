#include <sys/ioctl.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>

#define IOCTL_UPDATE_BUF _IOW('k', 1, struct user_arg)


// 测试程序示例
struct user_arg {
    char buf[128];
};

int main() {
    int fd = open("/dev/ioctl_example", O_RDWR);
    struct user_arg arg = {.buf = "Hello"};
    
    ioctl(fd, IOCTL_UPDATE_BUF, &arg);
    printf("Received: %s\n", arg.buf); // 应输出 "Hello from kernel"
    
    close(fd);
    return 0;
}