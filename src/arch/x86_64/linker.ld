ENTRY(_start)

SECTIONS
{
    /* 커널이 로드될 표준 물리 주소 (1MB 지점) */
    . = 1M;

    .text :
    {
        /* 커널의 실행 코드가 위치하는 섹션 */
        *(.text .text.*)
    }

    .rodata :
    {
        /* 읽기 전용 데이터 (상수 등) */
        *(.rodata .rodata.*)
    }

    .data :
    {
        /* 초기화된 전역 변수 */
        *(.data .data.*)
    }

    .bss :
    {
        /* 초기화되지 않은 전역 변수 (0으로 채워짐) */
        *(.bss .bss.*)
    }
}