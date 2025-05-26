obj-m += example_ioctl.o 
example_ioctl-objs += main.o ../../lib/lde.o

PWD = $(shell pwd)

build:
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) modules

user:
	gcc user.c -o user

clean:
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) clean

