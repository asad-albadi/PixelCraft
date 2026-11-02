
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000036708b <app::gen::GlobalPref::ColorBar::ColorBar(std::__cxx11::basic_string<char, std::char_traits<char>, std::allocator<char> > const&)+0x36b>:
  36708b:	48 8d 7c 24 18       	lea    0x18(%rsp),%rdi
  367090:	be ff 00 00 00       	mov    $0xff,%esi
  367095:	ba ff 00 00 00       	mov    $0xff,%edx
  36709a:	b9 ff 00 00 00       	mov    $0xff,%ecx
  36709f:	41 b8 ff 00 00 00    	mov    $0xff,%r8d
  3670a5:	e8 16 35 e0 ff       	call   16a5c0 <app::Color::fromRgb(int, int, int, int)>
  3670aa:	4c 8d b3 f8 01 00 00 	lea    0x1f8(%rbx),%r14
  3670b1:	48 89 9b 00 02 00 00 	mov    %rbx,0x200(%rbx)
  3670b8:	48 8d 05 8a 2f bd 00 	lea    0xbd2f8a(%rip),%rax        # f3a049 <gUnaryActiveEdge+0x68e5>
  3670bf:	48 89 83 08 02 00 00 	mov    %rax,0x208(%rbx)
  3670c6:	48 8d 2d 83 a1 93 01 	lea    0x193a183(%rip),%rbp        # 1ca1250 <vtable for app::Option<app::Color>+0x10>
  3670cd:	48 89 ab f8 01 00 00 	mov    %rbp,0x1f8(%rbx)
  3670d4:	4c 8d 2d c5 a1 93 01 	lea    0x193a1c5(%rip),%r13        # 1ca12a0 <vtable for obs::signal<void (app::Color const&), obs::default_list>+0x10>
  3670db:	4c 89 ab 10 02 00 00 	mov    %r13,0x210(%rbx)
  3670e2:	0f 57 c0             	xorps  %xmm0,%xmm0
  3670e5:	0f 11 83 18 02 00 00 	movups %xmm0,0x218(%rbx)
  3670ec:	0f 11 83 28 02 00 00 	movups %xmm0,0x228(%rbx)
  3670f3:	0f 11 83 38 02 00 00 	movups %xmm0,0x238(%rbx)
  3670fa:	c7 83 48 02 00 00 00 	movl   $0x0,0x248(%rbx)
  367101:	00 00 00
  367104:	4c 89 ab 50 02 00 00 	mov    %r13,0x250(%rbx)
  36710b:	0f 11 83 58 02 00 00 	movups %xmm0,0x258(%rbx)
  367112:	0f 11 83 68 02 00 00 	movups %xmm0,0x268(%rbx)
  367119:	0f 11 83 78 02 00 00 	movups %xmm0,0x278(%rbx)
  367120:	c7 83 88 02 00 00 00 	movl   $0x0,0x288(%rbx)
  367127:	00 00 00
  36712a:	48 8b 44 24 38       	mov    0x38(%rsp),%rax
  36712f:	48 89 83 b0 02 00 00 	mov    %rax,0x2b0(%rbx)
  367136:	0f 10 44 24 18       	movups 0x18(%rsp),%xmm0
  36713b:	0f 10 4c 24 28       	movups 0x28(%rsp),%xmm1
  367140:	0f 11 8b a0 02 00 00 	movups %xmm1,0x2a0(%rbx)
  367147:	0f 11 83 90 02 00 00 	movups %xmm0,0x290(%rbx)
  36714e:	0f 10 44 24 18       	movups 0x18(%rsp),%xmm0
  367153:	0f 10 4c 24 28       	movups 0x28(%rsp),%xmm1
  367158:	0f 11 83 b8 02 00 00 	movups %xmm0,0x2b8(%rbx)
  36715f:	0f 11 8b c8 02 00 00 	movups %xmm1,0x2c8(%rbx)
  367166:	48 8b 44 24 38       	mov    0x38(%rsp),%rax
  36716b:	48 89 83 d8 02 00 00 	mov    %rax,0x2d8(%rbx)
  367172:	c6 83 e0 02 00 00 00 	movb   $0x0,0x2e0(%rbx)
  367179:	48 8d 7c 24 18       	lea    0x18(%rsp),%rdi
  36717e:	31 f6                	xor    %esi,%esi
  367180:	31 d2                	xor    %edx,%edx
  367182:	31 c9                	xor    %ecx,%ecx
  367184:	41 b8 ff 00 00 00    	mov    $0xff,%r8d
  36718a:	e8 31 34 e0 ff       	call   16a5c0 <app::Color::fromRgb(int, int, int, int)>
  36718f:	48 89 9b f0 02 00 00 	mov    %rbx,0x2f0(%rbx)
  367196:	48 8d 05 c8 2e bd 00 	lea    0xbd2ec8(%rip),%rax        # f3a065 <gUnaryActiveEdge+0x6901>
  36719d:	48 89 83 f8 02 00 00 	mov    %rax,0x2f8(%rbx)
