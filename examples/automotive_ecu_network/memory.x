/* Memory layout for STM32F4xx-like microcontroller */
/* This defines where code and data should be placed in memory */

MEMORY
{
  /* Flash memory - where the program code is stored */
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
  
  /* RAM memory - where variables and stack are stored */
  RAM : ORIGIN = 0x20000000, LENGTH = 192K
}

/* The location of the stack can be overridden using the `_stack_start` symbol.
   Place the stack at the end of RAM */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* The location of the .text section can be overridden using the
   `_stext` symbol.  By default it will place after .vector_table */
/* _stext = ORIGIN(FLASH) + 0x40c; */
