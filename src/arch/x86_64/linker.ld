ENTRY(_start)

SECTIONS
{
/* Standard physical load address for the kernel (1 MiB). */
    . = 1M;

    .text :
    {
/* Section containing executable kernel code. */
        *(.text .text.*)
    }

    .rodata :
    {
/* Read-only data such as constants. */
        *(.rodata .rodata.*)
    }

    .data :
    {
/* Initialized global data. */
        *(.data .data.*)
    }

    .bss :
    {
/* Uninitialized globals (zero-filled at boot). */
        *(.bss .bss.*)
    }
}
