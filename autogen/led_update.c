char *rgb_led_update()
{
  _BYTE *v0; // r11
  int v1; // r8
  int v2; // r5
  int v3; // r6
  int v4; // r7
  unsigned __int8 *v5; // r4
  unsigned int i; // r10
  int v7; // r0
  int v8; // r1
  int v9; // r0
  char *result; // r0
  char *v11; // lr
  int v12; // r11
  unsigned int v13; // r0
  unsigned int v14; // r10
  int v15; // r3
  int v16; // r2
  int v17; // r1
  int v18; // r2

  v0 = g_led_state_struct;
  v1 = g_anim_palette;
  v2 = g_led_buf_R;
  v3 = g_led_buf_G;
  v4 = g_led_buf_B;
  v5 = (unsigned __int8 *)g_led_routing_table;
  if ( *((char *)g_led_state_struct + 10) < (int)(unsigned __int8)*g_total_keys_ptr )
  {
    for ( i = 0; i < (unsigned __int8)*g_leds_per_key_ptr; i = (unsigned __int8)(i + 1) )
    {
      if ( (unsigned __int8)v0[14] < 8u )
        JUMPOUT(0x7320);
      v7 = 3 * (char)v0[10];
      v8 = v5[12 * (char)v0[10] + i];
      *(_BYTE *)(v2 + v8) = *(_BYTE *)(v1 + v7);
      v9 = v7 + v1;
      *(_BYTE *)(v3 + v8) = *(_BYTE *)(v9 + 1);
      *(_BYTE *)(v4 + v8) = *(_BYTE *)(v9 + 2);
      *(_BYTE *)(g_led_buf_alpha + v5[12 * (char)v0[10] + i]) = -1;
    }
    ++v0[10];
  }
  result = (char *)(unsigned __int8)v0[14];
  if ( result == byte_FF )
  {
    v0[10] = 0;
    result = g_total_keys_ptr;
    v11 = (char *)g_led_state_struct + 52;
    v12 = (unsigned __int8)*g_total_keys_ptr;
    while ( 1 )
    {
      v17 = *((char *)g_led_state_struct + 10);
      if ( v17 >= v12 )
        break;
      v13 = 0;
      v14 = (unsigned __int8)*g_leds_per_key_ptr;
      while ( v13 < v14 )
      {
        if ( v11[4] )
        {
          v15 = v5[12 * v17 + v13];
          *(_BYTE *)(v2 + v15) = *(_BYTE *)(v1 + 3 * v17);
          v16 = 3 * v17 + v1;
          *(_BYTE *)(v3 + v15) = *(_BYTE *)(v16 + 1);
          *(_BYTE *)(v4 + v15) = *(_BYTE *)(v16 + 2);
        }
        else
        {
          v18 = v5[12 * v17 + v13];
          *(_BYTE *)(v2 + v18) = v11[1];
          *(_BYTE *)(v3 + v18) = v11[2];
          *(_BYTE *)(v4 + v18) = v11[3];
        }
        *(_BYTE *)(g_led_buf_alpha + v5[12 * v17 + v13]) = -1;
        v13 = (unsigned __int8)(v13 + 1);
      }
      result = (char *)g_led_state_struct;
      *((_BYTE *)g_led_state_struct + 10) = v17 + 1;
    }
  }
  return result;
}