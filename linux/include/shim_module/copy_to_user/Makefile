# 定义所有需要编译的模块
obj-m += copy_to_user.o

# 内核源码目录
KDIR := ~/linux

# 指定架构和交叉编译器
ARCH := riscv
CROSS_COMPILE := riscv64-linux-gnu-

all:
	make -C $(KDIR) M=$(PWD) ARCH=$(ARCH) CROSS_COMPILE=$(CROSS_COMPILE) modules

clean:
	make -C $(KDIR) M=$(PWD) ARCH=$(ARCH) CROSS_COMPILE=$(CROSS_COMPILE) clean
