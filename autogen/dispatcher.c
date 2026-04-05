unsigned int __fastcall hid_command_dispatcher(int a1, int a2, int a3, int a4)
{
  _BYTE *v4; // r5
  unsigned int result; // r0
  int *v6; // r12
  unsigned int v7; // r0
  int *v8; // r8
  unsigned int v9; // r4
  int v10; // r3
  int v11; // r2
  int v12; // r7
  int v13; // r2
  unsigned int i; // r1
  unsigned int v15; // r7
  unsigned int v16; // r7
  char *v17; // r10
  int v18; // r7
  char *v19; // r7
  unsigned int v20; // r0

  v4 = g_state_ptr;
  result = *((unsigned __int8 *)g_state_ptr + 4);
  if ( !*((_BYTE *)g_state_ptr + 4) )
    return result;
  *((_BYTE *)g_state_ptr + 4) = 0;
  v6 = g_hid_input_ptr;
  result = *(unsigned __int8 *)g_hid_input_ptr;
  if ( result != 170 )
    goto LABEL_77;
  v7 = 0;
  v8 = g_hid_input_ptr - 16;
  do
  {
    *((_BYTE *)v8 + v7) = *((_BYTE *)v6 + v7);
    v7 = (unsigned __int8)(v7 + 1);
  }
  while ( v7 < 0x40 );
  *(_BYTE *)v8 = 85;
  result = *((unsigned __int8 *)v6 + 1);
  v9 = *((unsigned __int8 *)g_hid_input_ptr + 2);
  v10 = *((unsigned __int8 *)g_hid_input_ptr + 4);
  v11 = *((unsigned __int8 *)g_hid_input_ptr + 3);
  v12 = 39936;
  if ( result >> 4 != 1 )
  {
    if ( result >> 4 == 2 )
    {
      switch ( result )
      {
        case '!':
          v12 = 37376;
          break;
        case '"':
          v12 = 38400;
          break;
        case '#':
          v12 = 38912;
          break;
        case '$':
          v12 = 39424;
          break;
        default:
          if ( result != 37 )
          {
            switch ( result )
            {
              case '&':
                v12 = 45056;
                break;
              case '\'':
                v12 = 46592;
                break;
              case '(':
                v12 = 45568;
                break;
              default:
                return result;
            }
          }
          break;
      }
      if ( (unsigned int)(v11 + v12 + (v10 << 8)) >> 9 == (v11 + v9 + v12 + (v10 << 8)) >> 9 )
      {
        if ( v12 != 38400 && v12 != 46592 )
flash_write_sector:
          JUMPOUT(0x4A44);
      }
      else if ( v12 != 38400 && v12 != 46592 )
      {
        goto flash_write_sector;
      }
      JUMPOUT(0x260);
    }
    if ( result >> 4 == 6 )
    {
      if ( result != 100 )
      {
        switch ( result )
        {
          case 'e':
            JUMPOUT(0x24AC);
          case 'f':
            v4[2] = 1;
            break;
          case 'g':
            v4[2] = 0;
            break;
        }
        goto LABEL_77;
      }
sram_operation:
      JUMPOUT(0x292);
    }
    result = ~result << 28;
    if ( !result )
      goto sram_operation;
LABEL_77:
    v4[3] = 1;
    return result;
  }
  switch ( result )
  {
    case 0x10u:
      a2 = 36864;
      goto LABEL_16;
    case 0x11u:
      a2 = 37376;
      goto LABEL_16;
    case 0x12u:
      a2 = 38400;
      goto LABEL_16;
    case 0x13u:
      v15 = *(unsigned __int8 *)g_led_effect_ptr;
      *((_BYTE *)v8 + 8) = v15 + 1;
      if ( v15 >= 0xB )
        *((_BYTE *)v8 + 8) = v15;
      *((_BYTE *)v8 + 17) = *(_BYTE *)g_led_brightness_ptr;
      *((_BYTE *)v8 + 18) = *(_BYTE *)g_led_speed_ptr;
      *((_BYTE *)v8 + 19) = *(_BYTE *)g_led_extra_ptr;
      v16 = *(unsigned __int8 *)g_led_anim_type_ptr;
      if ( v16 >= 8 )
      {
        *((_BYTE *)v8 + 9) = -1;
        *((_BYTE *)v8 + 10) = -1;
        *((_BYTE *)v8 + 11) = -1;
        *((_BYTE *)v8 + 16) = 1;
      }
      else
      {
        v17 = (char *)g_static_color_lut_ptr;
        v18 = 3 * v16;
        *((_BYTE *)v8 + 9) = *((_BYTE *)g_static_color_lut_ptr + v18);
        v19 = &v17[v18];
        *((_BYTE *)v8 + 10) = v19[1];
        *((_BYTE *)v8 + 11) = v19[2];
        *((_BYTE *)v8 + 16) = 0;
      }
LABEL_16:
      if ( result != 19 )
      {
        v13 = v11 + a2 + (v10 << 8);
        for ( i = 0; v9 > i; i = (unsigned __int8)(i + 1) )
          *((_BYTE *)v8 + i + 8) = *(_BYTE *)(v13 + i);
      }
      if ( result == 16 )
      {
        if ( byte_9003 == 18 )
          goto flash_write_sector;
        v20 = *(_DWORD *)g_runtime_info_ptr;
        *((_WORD *)v8 + 11) = *(_DWORD *)g_runtime_info_ptr;
        *((_BYTE *)v8 + 24) = BYTE2(v20);
        result = HIBYTE(v20);
        *((_BYTE *)v8 + 25) = result;
        *((_BYTE *)v8 + 26) = 0;
      }
      goto LABEL_77;
    case 0x14u:
      a2 = 39424;
      goto LABEL_16;
    case 0x15u:
      a2 = 39936;
      goto LABEL_16;
    case 0x16u:
      a2 = 45056;
      goto LABEL_16;
    case 0x17u:
      a2 = 46592;
      goto LABEL_16;
    case 0x18u:
      a2 = 45568;
      goto LABEL_16;
  }
  return result;
}