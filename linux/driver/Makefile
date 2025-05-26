KDIR := /lib/modules/$(shell uname -r)/build

obj-m += simple_ioctl_driver.o

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean