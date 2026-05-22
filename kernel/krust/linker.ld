OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0xffffffff80000000;

    . = ALIGN(4K);
    .text :
    {
        KEEP(*(.text._start))
        *(.text .text.*)
    } :text

    . = ALIGN(4K);
    .rodata :
    {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(4K);
    .data :
    {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
        *(.data .data.*)
    } :data

    . = ALIGN(4K);
    .bss :
    {
        *(COMMON)
        *(.bss .bss.*)
    } :data

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame.*)
        *(.note .note.*)
        *(.comment .comment.*)
    }
}
