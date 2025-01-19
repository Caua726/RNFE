Olha, eu to vendo o vídeo [neste link](https://www.youtube.com/watch?v=F8kx56OZQhg&t=1183s)

Os operadores bitwise são cálculos lógicos entre bits binários. Eu estou vendo, até o momento, os operadores:

|  Nome | Operador | Descrição | Exemplos |
|-------|----------|-----------|----------|
|Exemplo|    ex    |Lorem Ipsum| 1 x 1 = x|
|       |          |           | 1 x 0 = x|
|       |          |           | 0 x 1 = x|
|       |          |           | 0 x 0 = x|
|  And  |    &     | Se for igual, será igual, se for diferente será zero | 1 & 1 = 1 
|       |          |           | 1 & 0 = 0|
|       |          |           | 0 & 1 = 0|
|       |          |           | 0 & 0 = 0|
|  OR   |    \|     | Se um dos bits for 1 o resultado vai ser 1 | 1 \| 1 = 1 |
|       |          |           |1 \| 0 = 1|
|       |          |           |0 \| 1 = 1|
|       |          |           |0 \| 0 = 0|
|  NOT  |    ~     | Ele inverte o bit de verdadeiro pra falso de falso</br> pra verdadeiro, de 0 pra 1 de 1 pra 0, e ele nao funciona</br> como um operador, ele é como uma função| 1 = 0 |
|       |          |           |  0 = 1   |
|  XOR  |    ^     | Exclusive OR, se os valores forem diferentes, retorna</br> 1, se forem iquais retorna 0 | 1 ^ 1 = 0 |
|       |          |           | 1 ^ 0 = 1 |
|       |          |           | 0 ^ 1 = 1 |
|       |          |           | 0 ^ 0 = 0 |
| SHIFT |  >>/<<   | Ele move o numero pra esqueda ou direita, ex:</br> x = 0100 e y = 1, x=(y<<4) = 1100, no caso eu movi o numero y 4 vezes </br> para a esquerda | x=1001 y=10 |
|       |          |           |x=(y<<2)=1101|
|       |          |           |x=(y>>1)=1001|