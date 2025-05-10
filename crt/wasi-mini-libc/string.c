#include "string.h"

extern size_t strlen(const char *str) {
  size_t out = 0;
  while (*str) {
    ++out;
    ++str;
  }
  return out;
}

extern char *strchr(const char *str, int ch) {
  while (*str) {
    if ((char) ch == *str)
      return (char *) str;
    else
      ++str;
  }
  return (char *) 0;
}

extern char *strrchr(const char *str, int ch) {
  const char *next = str;
  while ((next = strchr(str, ch)))
    str = next;
  return (char *) str;
}

extern size_t strspn(const char *str, const char *prefix) {
  size_t length = 0;
  while (str[length] && strchr(prefix, str[length]))
    ++length;
  return length;
}

extern size_t strcspn(const char *str, const char *cprefix) {
  size_t length = 0;
  while (str[length] && !strchr(cprefix, str[length]))
    ++length;
  return length;
}

extern char *strpbrk(const char *str, const char *breakset) {
  return (char *) (str + strcspn(str, breakset));
}

extern void *memset(void *ptr, int ch, size_t len) {
  unsigned char *ucptr = (unsigned char *) ptr;
  while (len) {
    *ucptr++ = (unsigned char) ch;
    --len;
  }

  return ptr;
}

extern char *strtok(char *str, const char *delims) {
  static char *curstr = (char *) 0;
  char *out;

  if (str)
    curstr = str;

  out = curstr + strspn(curstr, delims);
  curstr = out + strcspn(out, delims);

  *curstr = '\0';
  if (*out)
    return out;
  else
    return (char *) 0;
}

extern void *memcpy(void *dst, const void *src, size_t len) {
  unsigned char *ucdst = (unsigned char *) dst;
  unsigned char *ucsrc = (unsigned char *) src;
  while (len) {
    *ucdst++ = *ucsrc++;
    --len;
  }
  return dst;
}

extern char *strcpy(char *dst, const char *src) {
  memcpy(dst, src, strlen(src) + 1);
  return dst;
}

static size_t zmin(size_t a, size_t b) {
  if (a < b) {
    return a;
  } else {
    return b;
  }
}

extern char *strncpy(char *dst, const char *src, size_t len) {
  memcpy(dst, src, zmin(strlen(src), len) + 1);
  return dst;
}

extern int strcmp(const char *lhs, const char *rhs) {
  while (*lhs && *rhs) {
    if (*lhs != *rhs) {
      return *lhs - *rhs;
    }
  }

  return 0;
}

extern char *strcat(char *dest, const char *src) {
  char *tdst;
  tdst = dest + strlen(dest);
  (void) strcpy(tdst, src);
  return dest;
}


static void reverse(char str[], int length)
{
    int start = 0;
    int end = length - 1;
    while (start < end) {
        char temp = str[start];
        str[start] = str[end];
        str[end] = temp;
        end--;
        start++;
    }
}

/** returned pointer points to null temrinator of written string */
char* itoa(int num, char* str, int base)
{
    int i = 0;
    int isNegative = 0;
 
    /* Handle 0 explicitly, otherwise empty string is
     * printed for 0 */
    if (num == 0) {
        str[i++] = '0';
        str[i] = '\0';
        return str;
    }
 
    // In standard itoa(), negative numbers are handled
    // only with base 10. Otherwise numbers are
    // considered unsigned.
    if (num < 0 && base == 10) {
        isNegative = 1;
        num = -num;
    }
 
    // Process individual digits
    while (num != 0) {
        int rem = num % base;
        str[i++] = (rem > 9) ? (rem - 10) + 'a' : rem + '0';
        num = num / base;
    }
 
    // If number is negative, append '-'
    if (isNegative)
        str[i++] = '-';
 
    str[i] = '\0'; // Append string terminator
 
    // Reverse the string
    reverse(str, i);
 
    return &str[i];
}
