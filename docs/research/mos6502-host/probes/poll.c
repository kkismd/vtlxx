#define _GNU_SOURCE
#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <poll.h>
#include <errno.h>
#include <signal.h>
#include <sys/time.h>
#include <string.h>
static void sig(int n){(void)n;}
int main(void){int f[2];char c;struct pollfd p;int n,r,e;
pipe(f);write(f[1],"Q",1);close(f[1]);p=(struct pollfd){f[0],POLLIN,0};n=poll(&p,1,0);r=read(f[0],&c,1);printf("hup-data poll=%d revents=%#x read=%d byte=%c\n",n,p.revents,r,c);p.revents=0;n=poll(&p,1,0);r=read(f[0],&c,1);printf("hup-empty poll=%d revents=%#x read=%d\n",n,p.revents,r);close(f[0]);
pipe(f);fcntl(f[0],F_SETFL,fcntl(f[0],F_GETFL)|O_NONBLOCK);write(f[1],"R",1);p=(struct pollfd){f[0],POLLIN,0};n=poll(&p,1,0);read(f[0],&c,1);errno=0;r=read(f[0],&c,1);e=errno;printf("race-nonblock poll=%d revents=%#x read=%d errno=%d %s\n",n,p.revents,r,e,strerror(e));close(f[0]);close(f[1]);
pipe(f);int dead=f[0];close(f[0]);p=(struct pollfd){dead,POLLIN,0};n=poll(&p,1,0);printf("closed-fd poll=%d revents=%#x\n",n,p.revents);close(f[1]);
pipe(f);close(f[0]);p=(struct pollfd){f[1],POLLOUT,0};n=poll(&p,1,0);printf("broken-pipe-write poll=%d revents=%#x\n",n,p.revents);close(f[1]);
pipe(f);struct sigaction a={0};a.sa_handler=sig;sigaction(SIGALRM,&a,0);struct itimerval t={0};t.it_value.tv_usec=50000;setitimer(ITIMER_REAL,&t,0);p=(struct pollfd){f[0],POLLIN,0};errno=0;n=poll(&p,1,500);e=errno;printf("signal poll=%d errno=%d %s\n",n,e,strerror(e));close(f[0]);close(f[1]);return 0;}
