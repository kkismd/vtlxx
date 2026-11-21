#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include "vrEmu6502.h"
static uint8_t m[65536];
static uint8_t rd(uint16_t a,bool d){(void)d;return m[a];}
static void wr(uint16_t a,uint8_t v){m[a]=v;}
static unsigned word(unsigned a){return m[a]|m[(a+1)&65535]<<8;}
int main(int argc,char**argv){FILE*f=fopen(argv[1],"rb");uint8_t h[12];fread(h,1,12,f);unsigned load=h[8]|h[9]<<8,pc=h[10]|h[11]<<8,zp=h[7];fread(m+load,1,0xfff1-load,f);fclose(f);VrEmu6502*c=vrEmu6502New(CPU_6502,rd,wr);vrEmu6502Reset(c);vrEmu6502SetPC(c,pc);unsigned long long cycles=0, hooks[9]={0};for(;;){pc=vrEmu6502GetPC(c);if(pc>=0xfff1&&pc<=0xfff9){hooks[pc-0xfff1]++;if(pc==0xfff9){fprintf(stderr,"cycles=%llu read=%llu write=%llu exit=%llu\n",cycles,hooks[5],hooks[6],hooks[8]);return vrEmu6502GetAcc(c);}if(pc!=0xfff6&&pc!=0xfff7){fprintf(stderr,"unsupported %04x\n",pc);return 125;}unsigned n=vrEmu6502GetAcc(c)|(vrEmu6502GetX(c)<<8), sp=word(zp),buf=word(sp),fd=word(sp+2);sp+=4;m[zp]=sp;m[zp+1]=sp>>8;unsigned r=0;if(pc==0xfff6&&fd==0){while(r<n){int x=getchar();if(x==EOF)break;m[(buf+r++)&65535]=x;break;}}else if(pc==0xfff7&&(fd==1||fd==2)){r=fwrite(m+buf,1,n,fd==1?stdout:stderr);fflush(fd==1?stdout:stderr);}else return 125; m[0xffe0]=0xa9;m[0xffe1]=r;m[0xffe2]=0xa2;m[0xffe3]=r>>8;m[0xffe4]=0x60;vrEmu6502SetPC(c,0xffe0);}cycles+=vrEmu6502InstCycle(c);if(cycles>200000000)return 126;}}
