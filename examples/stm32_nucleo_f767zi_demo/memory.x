/* Memory layout for STM32F767ZI */
MEMORY
{
  /* Flash memory - 2MB total */
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  
  /* SRAM1 - 368KB (main RAM) */
  RAM : ORIGIN = 0x20020000, LENGTH = 368K
  
  /* SRAM2 - 16KB (additional RAM) */
  RAM2 : ORIGIN = 0x2007C000, LENGTH = 16K
  
  /* DTCM RAM - 128KB (tightly coupled, fastest access) */
  DTCM : ORIGIN = 0x20000000, LENGTH = 128K
  
  /* ITCM RAM - 16KB (instruction tightly coupled) */
  ITCM : ORIGIN = 0x00000000, LENGTH = 16K
}

/* Use main RAM for stack */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* CRDTosphere data can use SRAM2 for isolation */
SECTIONS
{
  .crdt_data (NOLOAD) : ALIGN(4)
  {
    *(.crdt_data .crdt_data.*)
  } > RAM2
}
