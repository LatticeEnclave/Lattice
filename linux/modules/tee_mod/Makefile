obj-m += teectl.o
teectl-objs += main.o

PWD = $(shell pwd)

build:
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) modules

dev:
	make -C ../linux-headers/build M=$(PWD) modules

test:
	sudo rmmod teectl.ko
	cp /mnt/shared/tee_mod/* ./
	make clean
	make build 
	sudo insmod teectl.ko
	sudo ./a.out

clean:
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) clean

