import tkinter as tk
from tkinter import font
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.chrome.options import Options
from webdriver_manager.chrome import ChromeDriverManager

# Dicionário de emails e senhas
email_senhas = {
    "contato@financeira.knxbrasil.com.br": "B@knxbrasil881063",
    "contato@knxbrasil.com.br": "B@knx881063",
    "knxbrasil@knxbrasil.com.br": "B@knx881063",
    "usuario4@example.com": "senhaABC",
}

def obterSenha(email):
    """Retorna a senha correspondente a um email, se existir."""
    return email_senhas.get(email)

def acessar_email(email):
    """
    Abre o navegador e faz login no webmail usando Selenium.
    O navegador permanece aberto após o login.
    """
    senha = obterSenha(email)
    if not senha:
        print("Email não encontrado!")
        return

    chrome_options = Options()
    chrome_options.add_argument("--start-maximized")
    service = Service(ChromeDriverManager().install())

    driver = webdriver.Chrome(service=service, options=chrome_options)
    try:
        driver.get("https://mail.hostinger.com/")
        driver.find_element(By.ID, "rcmloginuser").send_keys(email)
        driver.find_element(By.ID, "rcmloginpwd").send_keys(senha)
        driver.find_element(By.ID, "rcmloginpwd").send_keys(Keys.RETURN)
        print("Login realizado com sucesso!")
    except Exception as e:
        print(f"Erro ao acessar o email: {e}")
    # Não fechamos o navegador para manter a sessão aberta.

class EmailApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Exemplo GUI - Tkinter")
        self.root.geometry("500x350")
        self.root.configure(bg="#fafafa")  # Cor de fundo

        # Fonte customizada (opcional)
        self.title_font = font.Font(family="Arial", size=34, weight="bold")
        self.text_font = font.Font(family="Arial", size=31)

        # Label de título
        title_label = tk.Label(
            root,
            text="Selecione um email para visualizar e acessar o webmail:",
            bg="#fafafa",
            font=self.title_font
        )
        title_label.pack(pady=(15, 5))

        # Frame para agrupar Listbox e Scrollbar
        list_frame = tk.Frame(root, bg="#fafafa")
        list_frame.pack(padx=20, pady=5, fill=tk.BOTH, expand=True)

        # Scrollbar
        scrollbar = tk.Scrollbar(list_frame, orient=tk.VERTICAL)
        scrollbar.pack(side=tk.RIGHT, fill=tk.Y)

        # Listbox (ligada ao scrollbar)
        self.listbox = tk.Listbox(
            list_frame,
            yscrollcommand=scrollbar.set,
            font=self.text_font,
            width=50,
            height=6
        )
        self.listbox.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        scrollbar.config(command=self.listbox.yview)

        # Adicionar emails à Listbox
        for email in email_senhas:
            self.listbox.insert(tk.END, email)

        # Vincular o evento de seleção do Listbox
        self.listbox.bind("<<ListboxSelect>>", self.on_item_selected)

        # Frame para campos de exibição (Email e Senha)
        info_frame = tk.Frame(root, bg="#fafafa")
        info_frame.pack(pady=(10, 5))

        # 1) Campo para exibir o Email
        tk.Label(info_frame, text="Email selecionado:", font=self.text_font, bg="#fafafa").grid(
            row=0, column=0, sticky="e", padx=5, pady=2
        )
        self.email_var = tk.StringVar()
        self.email_entry = tk.Entry(info_frame, textvariable=self.email_var, font=self.text_font, width=35)
        self.email_entry.grid(row=0, column=1, pady=2)

        # 2) Campo para exibir a Senha
        tk.Label(info_frame, text="Senha:", font=self.text_font, bg="#fafafa").grid(
            row=1, column=0, sticky="e", padx=5, pady=2
        )
        self.senha_var = tk.StringVar()
        self.senha_entry = tk.Entry(info_frame, textvariable=self.senha_var, font=self.text_font, width=35)
        self.senha_entry.grid(row=1, column=1, pady=2)

        # Botão para acessar o email (abrir navegador)
        btn_acessar = tk.Button(
            root,
            text="Acessar",
            command=self.on_acessar,
            font=self.text_font,
            bg="#4caf50",
            fg="white",
            bd=0,
            padx=20,
            pady=5
        )
        btn_acessar.pack(pady=(10, 15))

    def on_item_selected(self, event):
        """
        Quando o usuário clica em um email na lista, exibimos o email e a senha
        nos Entries, mas só se realmente houver um item selecionado.
        """
        selection = self.listbox.curselection()
        if len(selection) == 0:
            # Se clicou em área vazia ou clicou de novo e removeu a seleção,
            # não faz nada (mantém o texto anterior).
            return

        email = self.listbox.get(selection[0])
        senha = obterSenha(email)
        self.email_var.set(email)
        self.senha_var.set(senha)

    def on_acessar(self):
        """Chamada ao clicar no botão 'Acessar' para abrir o navegador."""
        selection = self.listbox.curselection()
        if selection:
            email = self.listbox.get(selection[0])
            print(f"Abrindo o navegador para: {email}")
            acessar_email(email)
        else:
            print("Nenhum email selecionado!")

if __name__ == "__main__":
    root = tk.Tk()
    app = EmailApp(root)
    root.mainloop()
